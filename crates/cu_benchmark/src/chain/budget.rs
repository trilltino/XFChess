use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_sdk::instruction::Instruction;

pub fn apply_compute_budget(
    ixs: &mut Vec<Instruction>,
    cu_limit: u32,
    cu_price: u64,
    heap_size: u32,
) {
    ixs.splice(
        0..0,
        [
            ComputeBudgetInstruction::set_compute_unit_limit(cu_limit),
            ComputeBudgetInstruction::set_compute_unit_price(cu_price),
            ComputeBudgetInstruction::request_heap_frame(heap_size),
        ],
    );
}
