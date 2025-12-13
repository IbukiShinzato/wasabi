extern crate alloc;

use crate::result::Result;
use crate::uefi::EfiMemoryDescriptor;
use crate::uefi::EfiMemoryType;
use crate::uefi::MemoryMapHolder;
use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use alloc::boxed::Box;
use core::borrow::BorrowMut;
use core::cell::RefCell;
use core::cmp::max;
use core::fmt;
use core::mem::size_of;
use core::ops::DerefMut;
use core::ptr::null_mut;

// v以上の最も近い2のべき乗を求める関数
pub fn round_up_to_nearest_pow2(v: usize) -> Result<usize> {
    1_usize
        .checked_shl(usize::BITS - v.wrapping_sub(1).leading_zeros())
        .ok_or("Out of range")
}

struct Header {
    next_header: Option<Box<Header>>, // 次の空きブロックへのスマートポインタ
    size: usize,                      // このHeaderが管理するメモリブロックの「データ領域」のサイズ
    is_allocated: bool,               // このブロックが割り当て済み（true）か空き（false）か
    _reserved: usize,
}
const HEADER_SIZE: usize = size_of::<Header>(); // Header構造体自体のサイズ (32バイト)
#[allow(clippy::assertions_on_constants)]
const _: () = assert!(HEADER_SIZE == 32); // ヘッダーサイズが32バイトであることを保証
const _: () = assert!(HEADER_SIZE.count_ones() == 1); // ヘッダーサイズが2のべき乗であることを保証
pub const LAYOUT_PAGE_4K: Layout = unsafe { Layout::from_size_align_unchecked(4096, 4096) }; // 4KBページのレイアウト定義

impl Header {
    // メモリ割り当てが可能かどうかの確認
    // size: ユーザーが要求しているデータ領域のサイズ
    // align: ユーザーが要求しているメモリのアラインメント
    // 純粋な要求サイズ(size)と、アライメント調整による最大オーバーヘッド(align)、
    // そして新しく切り出すブロック用とパディング領域用の2つのヘッダー(HEADER_SIZE * 2)
    // の合計を含めても、現在の空き領域は十分かどうかの判定（安全側の概算チェック）
    fn can_provide(&self, size: usize, align: usize) -> bool {
        self.size >= size + HEADER_SIZE * 2 + align
    }
    fn is_allocated(&self) -> bool {
        self.is_allocated
    }
    // メモリブロックの終了アドレス (データ領域の直後のアドレス)
    // self (Headerのポインタ)をusizeに変換し、それにHeaderが持つ「データ領域のサイズ」を足して、
    // Header + データ領域の全体の終了アドレスを求めている。
    fn end_addr(&self) -> usize {
        self as *const Header as usize + self.size
    }
    // 指定されたアドレスに新しいHeader構造体を配置・初期化する (unsafe操作)
    // addr: Headerを配置したいメモリ上のアドレス
    unsafe fn new_from_addr(addr: usize) -> Box<Header> {
        let header = addr as *mut Header;
        header.write(Header {
            next_header: None,
            size: 0,
            is_allocated: false,
            _reserved: 0,
        });
        Box::from_raw(addr as *mut Header)
    }
    // 割り当て済みブロックのデータ領域開始アドレスからHeaderのアドレスを逆算する
    // addr: データ領域が始まるアドレス
    // Header(HEADER_SIZE)はデータ領域よりも前に配置されているため、
    // addrからHEADER_SIZE分だけ引くことで、Headerの開始アドレスを取得している。
    unsafe fn from_allocated_region(addr: *mut u8) -> Box<Header> {
        let header = addr.sub(HEADER_SIZE) as *mut Header;
        Box::from_raw(header)
    }
    // メモリ割り当てのメインロジック
    fn provide(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        // sizeとalignをHEADER_SIZEの倍数などに丸める
        let size = max(round_up_to_nearest_pow2(size).ok()?, HEADER_SIZE);
        let align = max(align, HEADER_SIZE);

        // 現在のブロックが割り当て済みか、必要なサイズ・アライメントを満たさない場合は割り当て不可
        if self.is_allocated() || !self.can_provide(size, align) {
            None
        } else {
            // 使用したサイズ
            let mut size_used = 0;

            // 割り当て開始アドレスの計算: 空きブロックの末尾から領域を切り出し、alignの倍数に切り下げる
            let allocated_addr = (self.end_addr() - size) & !(align - 1);

            // 割り当てる領域用のHeaderを、allocated_addrの直前（- HEADER_SIZE）に配置
            let mut header_for_allocated =
                unsafe { Self::new_from_addr(allocated_addr - HEADER_SIZE) };
            header_for_allocated.is_allocated = true;
            size_used += header_for_allocated.size;
            header_for_allocated.next_header = self.next_header.take();

            // 隙間（パディング）の処理: アライメント調整によってselfの末尾と新ブロックの間に隙間ができた場合
            if header_for_allocated.end_addr() != self.end_addr() {
                // 隙間（パディング領域）用のHeaderを作成し、割り当て済みとしてマーク
                let mut header_for_padding =
                    unsafe { Self::new_from_addr(header_for_allocated.end_addr()) };
                header_for_padding.is_allocated = true;
                // パディング領域のサイズを計算 (selfの末尾 - 新ブロックの末尾)
                header_for_padding.size = self.end_addr() - header_for_allocated.end_addr();
                size_used += header_for_padding.size;
                header_for_padding.next_header = header_for_allocated.next_header.take();
                header_for_allocated.next_header = Some(header_for_padding);
            }
            // 元の空きブロック (self) を縮小し、新しく切り出したブロックをリストに繋ぐ
            assert!(self.size >= size_used + HEADER_SIZE);
            self.size -= size_used;
            self.next_header = Some(header_for_allocated);
            Some(allocated_addr as *mut u8)
        }
    }
}
// panicさせることによって自動解放を防ぐ
impl Drop for Header {
    fn drop(&mut self) {
        panic!("Header should not be dropped!");
    }
}
// デバッグ用のトレイト実装
impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Header @ {:#018X} {{ size: {:#018X}, is_allocated: {} }}",
            self as *const Header as usize,
            self.size,
            self.is_allocated()
        )
    }
}

// ヒープメモリ全体を管理するコンテナ
pub struct FirstFitAllocator {
    // 空きメモリブロックの連結リストの先頭 (Headerへのスマートポインタ) を格納。
    // RefCellにより、静的変数（イミュータブル）でも内部のデータを可変に扱う（書き換える）ことを可能にしている。
    first_header: RefCell<Option<Box<Header>>>,
}

// ここでglobal_allocatorアトリビュートを設定することによって、
// Rustプログラム全体（Box, Vec, Stringなど）のメモリの確保・解放をこの静的変数ALLOCATORに依頼するようになる。
#[global_allocator]
pub static ALLOCATOR: FirstFitAllocator = FirstFitAllocator {
    first_header: RefCell::new(None),
};

// 複数のスレッドから安全に共有できるとコンパイラに宣言するためのトレイト（ここではunsafeで仮定）
unsafe impl Sync for FirstFitAllocator {}

unsafe impl GlobalAlloc for FirstFitAllocator {
    // メモリの確保（GlobalAllocインターフェース）
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_with_options(layout)
    }

    // メモリの解放（GlobalAllocインターフェース）
    // ptr: ユーザーから返されたデータ領域の開始アドレス
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // 1. データアドレスから、その直前のHeaderを逆算して取得し、Boxで管理下に置く。
        let mut region = Header::from_allocated_region(ptr);

        // 2. 解放処理の第一段階として、割り当てフラグを解除し、空きに戻す。
        region.is_allocated = false;

        // 3. Boxの所有権を意図的に放棄（leak）することで、Headerのdrop（panic!）を防ぎ、
        //    Header構造体をメモリ上に残し、後で空きリストに再挿入できるようにする。
        Box::leak(region);
        // Note: この後、`dealloc`メソッドの続きで空きリストへの再挿入処理が行われるはず。
    }
}

impl FirstFitAllocator {
    // 最初の割り当てられるブロックの探索と割り当てを実行するメソッド。
    // 連結リストを先頭から順に辿り、要求サイズを格納できる空きブロックの探索（First-Fitアルゴリズム）。
    pub fn alloc_with_options(&self, layout: Layout) -> *mut u8 {
        // RefCellからfirst_headerへの可変参照を取得。ループでポインタを更新するため複雑な手続きが必要。
        let mut header = self.first_header.borrow_mut();
        let mut header = header.deref_mut();

        loop {
            match header {
                // 空きブロック（Header）が存在する場合
                Some(e) => match e.provide(layout.size(), layout.align()) {
                    // provideが成功した場合
                    Some(p) => break p, // 割り当てられたデータ領域のアドレスを返す。
                    // provideが失敗した場合（サイズ不足など）
                    None => {
                        // 次の空きブロックのHeaderへポインタを移動し、ループ続行。
                        header = e.next_header.borrow_mut();
                        continue;
                    }
                },
                // リストの終端（None）に到達した場合
                None => break null_mut::<u8>(), // 空き容量なしとしてnullポインタを返す。
            }
        }
    }

    // OSが起動した直後、ブートローダから渡されたメモリマップを基にヒープを初期化し、
    // 利用可能な物理メモリ領域をアロケータの空きリストに登録する。
    pub fn init_with_mmap(&self, memory_map: &MemoryMapHolder) {
        for e in memory_map.iter() {
            // CONVENTIONAL_MEMORY（OSが自由に使える空きメモリ）だけを選別する。
            if e.memory_type() != EfiMemoryType::CONVENTIONAL_MEMORY {
                continue;
            }
            self.add_free_from_descriptor(e);
        }
    }

    // UEFIのメモリ記述子（Descriptor）を基に、実際の物理アドレスにHeaderを割り当て、空きリストに登録する。
    fn add_free_from_descriptor(&self, desc: &EfiMemoryDescriptor) {
        let mut start_addr = desc.physical_start() as usize;
        let mut size = desc.number_of_pages() as usize * 4096;

        // アドレス0からの割り当てを防ぐための処理（最初の4KBは予約または問題があることが多いため）
        if start_addr == 0 {
            start_addr += 4096;
            size = size.saturating_add(4096);
        }
        if size <= 4096 {
            return; // 4KB以下の領域は無視
        }

        // 1. 物理アドレスの先頭に、新しい空きブロック用のHeaderを強制的に書き込む。
        let mut header = unsafe { Header::new_from_addr(start_addr) };
        header.next_header = None;
        header.is_allocated = false; // 空きとしてマーク
        header.size = size; // 記述子から得たサイズをHeaderに設定

        // 2. 新しいブロックを空きリストの先頭に挿入（プッシュ）。
        let mut first_header = self.first_header.borrow_mut();
        let prev_last = first_header.replace(header); // 現在の先頭を退避させ、新しいHeaderを先頭に設定
        drop(first_header); // 一時的な可変参照を解放

        // 3. 新しい先頭のnext_headerを、以前の先頭（prev_last）に繋ぎ直す。
        let mut header = self.first_header.borrow_mut();
        header.as_mut().unwrap().next_header = prev_last;
    }
}
