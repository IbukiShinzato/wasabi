#![no_std]
#![no_main]

use core::arch::asm;
use core::cmp::min;
use core::mem::offset_of;
use core::mem::size_of;
use core::panic::PanicInfo;
use core::ptr::null_mut;

type EfiVoid = u8;
type EfiHandle = u64;
type Result<T> = core::result::Result<T, &'static str>;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct EfiGuid {
    data0: u32,
    data1: u16,
    data2: u16,
    data3: [u8; 8],
}

const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data0: 0x9042a9de,
    data1: 0x23dc,
    data2: 0x4a38,
    data3: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[must_use]
#[repr(u64)]
enum EfiStatus {
    Success = 0,
}

#[repr(C)]
// EFIブートサービステーブル
struct EfiBootServicesTable {
    _reserved0: [u64; 40],
    // x86_64環境では関数呼び出し規約がWindows ABIに従うため、extern "win64"を指定したいがRustではサポートされていないため、extern "C"を使用する
    locate_protocol: extern "C" fn(
        protocol: *const EfiGuid,
        registration: *const EfiVoid,
        interface: *mut *mut EfiVoid,
    ) -> EfiStatus,
}
// offset_of!マクロを使用することによって、locate_protocolのオフセットが320であることを確認する
const _: () = assert!(offset_of!(EfiBootServicesTable, locate_protocol) == 320);

#[repr(C)]
// EFIシステムテーブル
struct EfiSystemTable {
    _reserved0: [u64; 12],
    pub boot_services: &'static EfiBootServicesTable,
}
// boot_servicesのオフセットが96であることを確認する
const _: () = assert!(offset_of!(EfiSystemTable, boot_services) == 96);

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocolPixelInfo {
    version: u32,
    // 水平方向の画素数
    pub horizontal_resolution: u32,
    // 垂直方向の画素数
    pub vertical_resolution: u32,
    _padding0: [u32; 5],
    pub pixels_per_scan_line: u32,
}
// EfiGraphicsOutputProtocolPixelInfoのサイズが36バイトであることを確認する
const _: () = assert!(size_of::<EfiGraphicsOutputProtocolPixelInfo>() == 36);

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocolMode<'a> {
    pub max_mode: u32,
    pub mode: u32,
    pub info: &'a EfiGraphicsOutputProtocolPixelInfo,
    pub size_of_info: u64,
    // フレームバッファの開始アドレスとサイズ
    pub frame_buffer_base: u64,
    pub frame_buffer_size: u64,
}

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocol<'a> {
    reserved: [u64; 3],
    pub mode: &'a EfiGraphicsOutputProtocolMode<'a>,
}
fn locate_graphic_protocol<'a>(
    efi_system_table: &EfiSystemTable,
) -> Result<&'a EfiGraphicsOutputProtocol<'a>> {
    // null_mut()で空のポインタを作る
    let mut graphic_output_protocol = null_mut::<EfiGraphicsOutputProtocol>();
    // locate_protocol関数を呼び出して、グラフィックス出力プロトコルのアドレスを取得する
    let status = (efi_system_table.boot_services.locate_protocol)(
        // 検索したいプロトコルのGUIDへのポインタ
        &EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
        // C言語のNULLポインタに相当
        null_mut::<EfiVoid>(),
        &mut graphic_output_protocol as *mut *mut EfiGraphicsOutputProtocol as *mut *mut EfiVoid,
    );

    if status != EfiStatus::Success {
        return Err("Failed to locate graphics output protocol");
    }

    Ok(unsafe { &*graphic_output_protocol })
}

pub fn hlt() {
    unsafe {
        asm!("hlt");
    }
}

#[no_mangle]
fn efi_main(_image_handle: EfiHandle, efi_system_table: &EfiSystemTable) {
    let mut vram = init_vram(efi_system_table).expect("init_vram failed");

    let vw = vram.width;
    let vh = vram.height;
    fill_rect(&mut vram, 0x000000, 0, 0, vw, vh).expect("fill_rect failed");
    fill_rect(&mut vram, 0xff0000, 32, 32, 32, 32).expect("fill_rect failed");
    fill_rect(&mut vram, 0x00ff00, 64, 64, 64, 64).expect("fill_rect failed");
    fill_rect(&mut vram, 0x0000ff, 128, 128, 128, 128).expect("fill_rect failed");
    for i in 0..256 {
        // 斜め線を描画
        // 0x010101はRGB各色が1ずつ増加することを意味する
        let _ = draw_point(&mut vram, 0x010101 * i as u32, i, i);
    }

    // 反対方向も作成
    fill_rect(&mut vram, 0xff0000, 192, 32, 32, 32).expect("fill_rect failed");
    fill_rect(&mut vram, 0x00ff00, 128, 64, 64, 64).expect("fill_rect failed");
    fill_rect(&mut vram, 0x0000ff, 0, 128, 128, 128).expect("fill_rect failed");
    for i in 0..256 {
        let _ = draw_point(&mut vram, 0x010101 * (256 - i) as u32, 256 - i, i);
    }

    let grid_size = 32;
    let rect_size = grid_size * 8;
    // rect_sizeまでをgrid_size飛ばしでループ
    for i in (0..=rect_size).step_by(grid_size as usize) {
        // 横の赤線(0xff0000)を引く
        let _ = draw_line(&mut vram, 0xff0000, 0, i, rect_size, i);
        // 縦の赤線(0xff0000)を引く
        let _ = draw_line(&mut vram, 0xff0000, i, 0, i, rect_size);
    }

    let cx = rect_size / 2;
    let cy = rect_size / 2;
    for i in (0..=rect_size).step_by(grid_size as usize) {
        let _ = draw_line(&mut vram, 0xffff00, cx, cy, 0, i);
        let _ = draw_line(&mut vram, 0x00ffff, cx, cy, i, 0);
        let _ = draw_line(&mut vram, 0xff00ff, cx, cy, rect_size, i);
        let _ = draw_line(&mut vram, 0xffffff, cx, cy, i, rect_size);
    }

    for (i, c) in "ABCDEF".chars().enumerate() {
        draw_font_fg(&mut vram, i as i64 * 16 + 256, i as i64 * 16, 0xffffff, c);
    }

    // println!("Hello, world!");
    draw_str_fg(&mut vram, 256, 256, 0xffffff, "Hello, World!");

    loop {
        hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        hlt();
    }
}

trait Bitmap {
    fn bytes_per_pixel(&self) -> i64;
    fn pixels_per_line(&self) -> i64;
    fn width(&self) -> i64;
    fn height(&self) -> i64;
    fn buf_mut(&mut self) -> *mut u8;

    // 指定した座標のピクセルへの可変ポインタを返す（範囲チェックなし）
    unsafe fn unchecked_pixel_at_mut(&mut self, x: i64, y: i64) -> *mut u32 {
        self.buf_mut()
            .add(((y * self.pixels_per_line() + x) * self.bytes_per_pixel()) as usize)
            as *mut u32
    }

    fn pixel_at_mut(&mut self, x: i64, y: i64) -> Option<&mut u32> {
        // 範囲チェックを行う
        if self.is_in_x_range(x) && self.is_in_y_range(y) {
            unsafe { Some(&mut *(self.unchecked_pixel_at_mut(x, y))) }
        } else {
            None
        }
    }

    // x座標が範囲内かどうかを返す
    fn is_in_x_range(&self, px: i64) -> bool {
        // 範囲チェックの上限は幅と1ラインあたりのピクセル数の小さい方にする
        0 <= px && px < min(self.width(), self.pixels_per_line())
    }

    // y座標が範囲内かどうかを返す
    fn is_in_y_range(&self, py: i64) -> bool {
        // 範囲チェック
        0 <= py && py < self.height()
    }
}

// VRAMの情報を保持する構造体
#[derive(Clone, Copy)]
struct VramBufferInfo {
    buf: *mut u8,
    width: i64,
    height: i64,
    pixels_per_line: i64,
}

// BitmapトレイトをVramBufferInfo構造体に実装する
impl Bitmap for VramBufferInfo {
    fn bytes_per_pixel(&self) -> i64 {
        4
    }
    fn pixels_per_line(&self) -> i64 {
        self.pixels_per_line
    }
    fn width(&self) -> i64 {
        self.width
    }
    fn height(&self) -> i64 {
        self.height
    }
    fn buf_mut(&mut self) -> *mut u8 {
        self.buf
    }
}

fn init_vram(efi_system_table: &EfiSystemTable) -> Result<VramBufferInfo> {
    let gp = locate_graphic_protocol(efi_system_table)?;
    Ok(VramBufferInfo {
        buf: gp.mode.frame_buffer_base as *mut u8,
        width: gp.mode.info.horizontal_resolution as i64,
        height: gp.mode.info.vertical_resolution as i64,
        pixels_per_line: gp.mode.info.pixels_per_scan_line as i64,
    })
}

unsafe fn unchecked_draw_point<T: Bitmap>(buf: &mut T, color: u32, x: i64, y: i64) {
    *buf.unchecked_pixel_at_mut(x, y) = color;
}

fn draw_point<T: Bitmap>(buf: &mut T, color: u32, x: i64, y: i64) -> Result<()> {
    *(buf.pixel_at_mut(x, y).ok_or("Out of Range")?) = color;
    Ok(())
}

fn fill_rect<T: Bitmap>(buf: &mut T, color: u32, px: i64, py: i64, w: i64, h: i64) -> Result<()> {
    if !buf.is_in_x_range(px)
        || !buf.is_in_y_range(py)
        || !buf.is_in_x_range(px + w - 1)
        || !buf.is_in_y_range(py + h - 1)
    {
        return Err("Out of Range");
    }
    for y in py..py + h {
        for x in px..px + w {
            unsafe {
                unchecked_draw_point(buf, color, x, y);
            }
        }
    }
    Ok(())
}

fn calc_slope_point(da: i64, db: i64, ia: i64) -> Option<i64> {
    if da < db {
        None
    } else if da == 0 {
        Some(0)
    } else if (0..=da).contains(&ia) {
        Some((2 * db * ia + da) / da / 2)
    } else {
        None
    }
}

fn draw_line<T: Bitmap>(buf: &mut T, color: u32, x0: i64, y0: i64, x1: i64, y1: i64) -> Result<()> {
    if !buf.is_in_x_range(x0)
        || !buf.is_in_x_range(x1)
        || !buf.is_in_y_range(y0)
        || !buf.is_in_y_range(y1)
    {
        return Err("Out of Range");
    }

    let dx = (x1 - x0).abs();
    let sx = (x1 - x0).signum();
    let dy = (y1 - y0).abs();
    let sy = (y1 - y0).signum();
    if dx >= dy {
        for (rx, ry) in (0..dx).flat_map(|rx| calc_slope_point(dx, dy, rx).map(|ry| (rx, ry))) {
            draw_point(buf, color, x0 + rx * sx, y0 + ry * sy)?;
        }
    } else {
        for (rx, ry) in (0..dy).flat_map(|ry| calc_slope_point(dy, dx, ry).map(|rx| (rx, ry))) {
            draw_point(buf, color, x0 + rx * sx, y0 + ry * sy)?;
        }
    }

    Ok(())
}

fn draw_font_fg<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32, c: char) {
    if let Some(font) = lookup_font(c) {
        for (dy, row) in font.iter().enumerate() {
            for (dx, pixel) in row.iter().enumerate() {
                let color = match pixel {
                    '*' => color,
                    _ => continue,
                };
                let _ = draw_point(buf, color, x + dx as i64, y + dy as i64);
            }
        }
    }
}

fn lookup_font(c: char) -> Option<[[char; 8]; 16]> {
    // fileの中身を取得
    const FONT_SOURCE: &str = include_str!("./font.txt");

    if let Ok(c) = u8::try_from(c) {
        // fileの中身を改行で分割
        let mut fi = FONT_SOURCE.split('\n');

        // 文字列がある行までloop
        while let Some(line) = fi.next() {
            // 文字列から"0x"を取り除く
            // デフォルトでは0x41の下にAのドット絵が描かれている
            // これを41のみにして10進数表記に変更
            if let Some(line) = line.strip_prefix("0x") {
                // 16進数表記 -> 10進数表記
                if let Ok(idx) = u8::from_str_radix(line, 16) {
                    if idx != c {
                        continue;
                    }
                    let mut font = [['*'; 8]; 16];
                    for (y, line) in fi.clone().take(16).enumerate() {
                        for (x, c) in line.chars().enumerate() {
                            // デフォルトでは全て'*'なので'.'に置き換えるところは置き換える
                            if let Some(e) = font[y].get_mut(x) {
                                *e = c;
                            }
                        }
                    }
                    return Some(font);
                }
            }
        }
    }

    None
}

// 文字列の入力を描く
fn draw_str_fg<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32, s: &str) {
    for (i, c) in s.chars().enumerate() {
        draw_font_fg(buf, x + i as i64 * 8, y, color, c)
    }
}
