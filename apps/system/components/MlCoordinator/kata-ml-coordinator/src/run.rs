#![no_std]

// ML Coordinator Design Doc: go/sparrow-ml-doc
extern crate kata_panic;

use core::slice;
use kata_logger::KataLogger;
use kata_ml_core::MlCore;
use kata_ml_interface::MlCoordinatorInterface;
use kata_ml_interface::MlCoreInterface;
use log::{error, info, trace};

pub struct MLCoordinator {
    is_loaded: bool,
    ml_core: MlCore,
}

extern "C" {
    static elf_file: *const u8;
}
// TODO(jesionowski): Get the size programatically.
const ELF_SIZE: usize = 0x300000;

pub static mut ML_COORD: MLCoordinator = MLCoordinator {
    is_loaded: false,
    ml_core: MlCore {},
};

impl MLCoordinator {
    fn init(&mut self) {
        self.ml_core.enable_interrupts(true);
    }

    fn handle_return_interrupt(&self) {
        extern "C" {
            fn finish_acknowledge() -> u32;
        }

        // TODO(hcindyl): check the return code and fault registers, move the result
        // from TCM to SRAM, update the input/model, and call mlcoord_execute again.
        let return_code = MlCore::get_return_code();
        let fault = MlCore::get_fault_register();

        if return_code != 0 {
            error!(
                "vctop execution failed with code {}, fault pc: {:#010X}",
                return_code, fault
            );
        }

        MlCore::clear_finish();
        assert!(unsafe { finish_acknowledge() == 0 });
    }
}

impl MlCoordinatorInterface for MLCoordinator {
    fn execute(&mut self) {
        if !self.is_loaded {
            let res = self
                .ml_core
                .load_elf(unsafe { slice::from_raw_parts(elf_file, ELF_SIZE) });
            if let Err(e) = res {
                error!("Load error: {:?}", e);
            } else {
                info!("Load successful.");
                self.is_loaded = true;
            }
        }

        if self.is_loaded {
            // Unhalt, start at default PC.
            self.ml_core.run();
        }
    }
}

#[no_mangle]
pub extern "C" fn pre_init() {
    static KATA_LOGGER: KataLogger = KataLogger;
    log::set_logger(&KATA_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}

#[no_mangle]
pub extern "C" fn mlcoord__init() {
    trace!("init");
    unsafe {
        ML_COORD.init();
    }
}

// TODO: Once multiple model support is in start by name.
#[no_mangle]
pub extern "C" fn mlcoord_execute() {
    unsafe {
        ML_COORD.execute();
    }
}

#[no_mangle]
pub extern "C" fn host_req_handle() {
    extern "C" {
        fn host_req_acknowledge() -> u32;
    }
    MlCore::clear_host_req();
    assert!(unsafe { host_req_acknowledge() == 0 });
}

#[no_mangle]
pub extern "C" fn finish_handle() {
    unsafe {
        ML_COORD.handle_return_interrupt();
    }
}

#[no_mangle]
pub extern "C" fn instruction_fault_handle() {
    extern "C" {
        fn instruction_fault_acknowledge() -> u32;
    }
    error!("Instruction fault in Vector Core.");
    MlCore::clear_instruction_fault();
    assert!(unsafe { instruction_fault_acknowledge() == 0 });
}

#[no_mangle]
pub extern "C" fn data_fault_handle() {
    extern "C" {
        fn data_fault_acknowledge() -> u32;
    }
    error!("Data fault in Vector Core.");
    MlCore::clear_data_fault();
    assert!(unsafe { data_fault_acknowledge() == 0 });
}
