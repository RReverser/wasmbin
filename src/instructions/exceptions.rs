use super::Instruction;
use crate::builtins::WasmbinCountable;
use crate::indices::{ExceptionId, LabelId};
use crate::io::{encode_decode_as, Wasmbin};
use crate::types::BlockType;
use crate::visit::Visit;

#[derive(Wasmbin)]
#[repr(u8)]
enum CatchRepr {
    Catch {
        exception: ExceptionId,
        target: LabelId,
    } = 0x00,
    CatchRef {
        exception: ExceptionId,
        target: LabelId,
    } = 0x01,
    CatchAll {
        target: LabelId,
    } = 0x02,
    CatchAllRef {
        target: LabelId,
    } = 0x03,
}

#[derive(WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct Catch {
    /// Whether to store an exception reference on the stack.
    pub catch_ref: bool,
    /// Catch a specific exception or any exception if set to `None`.
    pub exception_filter: Option<ExceptionId>,
    /// Target label.
    pub target: LabelId,
}

encode_decode_as!(Catch, {
    (Catch { catch_ref: false, exception_filter: Some(exception), target }) <=> (CatchRepr::Catch { exception, target }),
    (Catch { catch_ref: true, exception_filter: Some(exception), target }) <=> (CatchRepr::CatchRef { exception, target }),
    (Catch { catch_ref: false, exception_filter: None, target }) <=> (CatchRepr::CatchAll { target }),
    (Catch { catch_ref: true, exception_filter: None, target }) <=> (CatchRepr::CatchAllRef { target }),
});

#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct TryTable {
    pub block_type: BlockType,
    pub catches: Vec<Catch>,
    pub instructions: Vec<Instruction>,
}
