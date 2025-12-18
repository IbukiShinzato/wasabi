use crate::print;
use crate::println;
use crate::serial::SerialPort;
use core::fmt;
use core::mem::size_of;
use core::slice;

// ターミナル上への出力（シリアルポートを介して）
pub fn global_print(args: fmt::Arguments) {
    let mut writer = SerialPort::default();
    fmt::write(&mut writer, args).unwrap();
}

// u8のバイト列を16進数表示
fn hexdump_bytes(bytes: &[u8]) {
    let mut i = 0;
    let mut ascii = [0u8; 16];
    let mut offset = 0;
    for v in bytes.iter() {
        // データの開始位置からの距離
        // ８桁の16進数表示
        if i == 0 {
            print!("{offset:08X}: ");
        }
        // 1Byteずつ出力
        print!("{:02X} ", v);
        ascii[i] = *v;
        i += 1;
        if i == 16 {
            print!("|");
            // 1Byteずつ取得して、それを文字に変換
            for c in ascii.iter() {
                print!(
                    "{}",
                    match c {
                        // スペース(0x20)から'~'(0x7e)まで
                        // それ以外は'.'
                        0x20..=0x7e => {
                            *c as char
                        }
                        _ => {
                            '.'
                        }
                    }
                );
            }
            println!("|");
            offset += 16;
            i = 0;
        }
    }
    // バイト列が16個ない時
    if i != 0 {
        for _ in 0..(16 - i) {
            print!("   ");
        }
        print!("|");
        for c in ascii.iter().take(i) {
            print!(
                "{}",
                if (0x20u8..=0x7fu8).contains(c) {
                    *c as char
                } else {
                    '.'
                }
            )
        }
        println!("|");
    }
}
// どのような型(T)でも16進数のスライスに変換
// ポインタからデータを取得
// バイト単位でデータを読み取った方が操作しやすい
pub fn hexdump<T: Sized>(data: &T) {
    hexdump_bytes(unsafe { slice::from_raw_parts(data as *const T as *const u8, size_of::<T>()) });
}

// 改行なし出力
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::global_print(format_args!($($arg)*)));
}

// 改行あり出力
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// file!(): 呼び出されたファイル名
// line!(): 呼び出された行数

// ログメッセージ
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ($crate::print!("[INFO]  {}:{:<3}: {}\n", file!(), line!(), format_args!($($arg)*)))
}

// 警告メッセージ
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => ($crate::print!("[WARN]  {}:{:<3}: {}\n", file!(), line!(), format_args!($($arg)*)));
}

// エラーメッセージ
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ($crate::print!("[ERROR] {}:{:<3}: {}\n", file!(), line!(), format_args!($($arg)*)));
}
