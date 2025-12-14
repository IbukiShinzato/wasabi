use crate::qemu::exit_qemu;
use crate::qemu::QemuExitCode;
use crate::serial::SerialPort;
use core::any::type_name;
use core::fmt::Write;
use core::panic::PanicInfo;

pub trait TestTable {
    fn run(&self, writer: &mut SerialPort);
}
impl<T> TestTable for T
where
    T: Fn(),
{
    // テストの実行前と実行後にログ出力
    fn run(&self, writer: &mut SerialPort) {
        writeln!(writer, "[RUNNING] >>> {}", type_name::<T>()).unwrap();
        self();
        writeln!(writer, "[PASS   ] <<< {}", type_name::<T>()).unwrap();
    }
}

// テストの実行
pub fn test_runner(tests: &[&dyn TestTable]) -> ! {
    let mut sw = SerialPort::new_for_com1();
    writeln!(sw, "Running {} tests...", tests.len()).unwrap();
    for test in tests {
        test.run(&mut sw);
    }
    writeln!(sw, "Completed {} tests!", tests.len()).unwrap();
    exit_qemu(QemuExitCode::Success);
}

// パニックハンドラ
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut sw = SerialPort::new_for_com1();
    writeln!(sw, "PANIC during test: {info:?}").unwrap();
    exit_qemu(QemuExitCode::Fail);
}
