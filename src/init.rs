use crate::allocator::ALLOCATOR;
use crate::uefi::exit_from_efi_boot_services;
use crate::uefi::EfiHandle;
use crate::uefi::EfiSystemTable;
use crate::uefi::MemoryMapHolder;

// メモリマップの初期化
pub fn init_basic_runtime(
    image_handle: EfiHandle,
    efi_system_table: &EfiSystemTable,
) -> MemoryMapHolder {
    let mut memory_map = MemoryMapHolder::new();
    // UEFIブートサービスの終了
    exit_from_efi_boot_services(image_handle, efi_system_table, &mut memory_map);

    // アロケータの初期コード
    // OSが利用可能とマークされたメモリ（CONVENTIONAL_MEMORY)をアロケーターの空きリストに追加
    ALLOCATOR.init_with_mmap(&memory_map);
    memory_map
}
