/// Public CLI primitives for implementation-first command trees, parsed strategy dispatch, and strategy error types.
pub mod cmdkit;

pub use cmdkit::{CMDKitMaster, CMDKitMasterBuilder, ExecutionHandle, ThreadPoolCMDKitMaster};
