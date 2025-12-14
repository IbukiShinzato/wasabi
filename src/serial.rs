use crate::x86::busy_loop_hint;
use crate::x86::read_io_port_u8;
use crate::x86::write_io_port_u8;
use core::fmt;

pub struct SerialPort {
    base: u16,
}
impl SerialPort {
    pub fn new(base: u16) -> Self {
        Self { base }
    }

    pub fn new_for_com1() -> Self {
        // ほとんどのPCで標準とされているシリアルポート1番(COM1)のI/Oアドレス: 0x3f8
        Self::new(0x3f8)
    }

    // シリアルポートの初期化
    pub fn init(&mut self) {
        write_io_port_u8(self.base + 1, 0x00); // 割り込み無効化
        write_io_port_u8(self.base + 3, 0x80);

        // ボーレート設定: 通信速度を決めるための設定
        const BAUD_DIVISOR: u16 = 0x0001;
        write_io_port_u8(self.base, (BAUD_DIVISOR & 0xff) as u8);
        write_io_port_u8(self.base + 1, (BAUD_DIVISOR >> 8) as u8);

        // データビット長、ストップビットなどのデータ形式決定
        // FIFO制御レジスタを有効
        write_io_port_u8(self.base + 3, 0x03);
        write_io_port_u8(self.base + 2, 0xC7);
        write_io_port_u8(self.base + 4, 0x0B);
    }

    // 送信バッファが空になるまで待機し、一文字送信
    pub fn send_char(&self, c: char) {
        // base + 5: ラインステータスレジスタ
        // 0x20: 送信バッファ空きフラグ
        while (read_io_port_u8(self.base + 5) & 0x20) == 0 {
            // 送信可能になるまでCPUを休止
            busy_loop_hint();
        }
        // データレジスタ（base)に文字を書き込み送信
        write_io_port_u8(self.base, c as u8);
    }

    // 文字列をcharに分解して1文字ずつ送信
    pub fn send_str(&self, s: &str) {
        for c in s.chars() {
            self.send_char(c);
        }
    }
}

// Writeトレイト実装: write!/writeln!マクロを使えるようにする
impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let serial = Self::default();
        serial.send_str(s);
        Ok(())
    }
}

// Defaultトレイト実装: SerialPort::default()でCOM1インスタンスを作成可能にする
impl Default for SerialPort {
    fn default() -> Self {
        Self::new_for_com1()
    }
}
