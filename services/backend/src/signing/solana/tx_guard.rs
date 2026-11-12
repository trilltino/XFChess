use solana_sdk::{pubkey::Pubkey, transaction::Transaction};

use super::instructions::anchor_discriminator;

const MAX_INSTRUCTIONS: usize = 8;

pub fn validate_cosignable_tx(
    tx: &Transaction,
    program_id: &Pubkey,
    allowed: &[&str],
    required_accounts: &[Pubkey],
) -> Result<(), String> {
    let instructions = &tx.message.instructions;

    if instructions.is_empty() {
        return Err("transaction contains no instructions".to_string());
    }
    if instructions.len() > MAX_INSTRUCTIONS {
        return Err(format!(
            "transaction carries {} instructions; at most {MAX_INSTRUCTIONS} are accepted here",
            instructions.len()
        ));
    }

    let account_keys = &tx.message.account_keys;
    let allowed_discriminators: Vec<(&str, [u8; 8])> = allowed
        .iter()
        .map(|name| (*name, anchor_discriminator(name)))
        .collect();

    for (i, ix) in instructions.iter().enumerate() {
        let target = account_keys
            .get(ix.program_id_index as usize)
            .ok_or_else(|| format!("instruction {i} references an out-of-range program index"))?;

        if target == &solana_compute_budget_interface::ID {
            continue;
        }

        if target != program_id {
            return Err(format!(
                "instruction {i} targets program {target}, but only the XFChess program \
                 ({program_id}) may be co-signed here"
            ));
        }

        let discriminator = ix.data.get(..8).ok_or_else(|| {
            format!("instruction {i} is too short to carry an Anchor discriminator")
        })?;

        if !allowed_discriminators
            .iter()
            .any(|(_, d)| d.as_slice() == discriminator)
        {
            return Err(format!(
                "instruction {i} is not one of the instructions this endpoint may co-sign \
                 (allowed: {})",
                allowed.join(", ")
            ));
        }
    }

    for required in required_accounts {
        if !account_keys.contains(required) {
            return Err(format!(
                "transaction does not reference the expected account {required}; it does not \
                 correspond to the game named in this request"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;
    use solana_compute_budget_interface::ComputeBudgetInstruction;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        signature::{Keypair, Signer},
    };
    use solana_system_interface::instruction as system_instruction;

    fn program() -> Pubkey {
        Pubkey::new_unique()
    }

    fn ix_for(program_id: &Pubkey, name: &str, extra: &[AccountMeta]) -> Instruction {
        let mut data = anchor_discriminator(name).to_vec();
        data.extend_from_slice(&7u64.to_le_bytes());
        Instruction {
            program_id: *program_id,
            accounts: extra.to_vec(),
            data,
        }
    }

    fn tx_of(payer: &Keypair, ixs: &[Instruction]) -> Transaction {
        Transaction::new_unsigned(Message::new(ixs, Some(&payer.pubkey())))
    }

    #[test]
    fn accepts_the_real_create_game_setup_bundle() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();
        let game_pda = Pubkey::new_unique();
        let tx = tx_of(
            &payer,
            &[
                ix_for(
                    &program_id,
                    "create_game",
                    &[AccountMeta::new(game_pda, false)],
                ),
                ix_for(&program_id, "authorize_session_key", &[]),
            ],
        );

        validate_cosignable_tx(
            &tx,
            &program_id,
            &["create_game", "join_game", "authorize_session_key"],
            &[game_pda],
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("the bundle the game client actually sends must be accepted")?;
        Ok(())
    }

    #[test]
    fn accepts_compute_budget_instructions_before_setup() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();
        let game_pda = Pubkey::new_unique();
        let tx = tx_of(
            &payer,
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(250_000),
                ix_for(
                    &program_id,
                    "create_game",
                    &[AccountMeta::new(game_pda, false)],
                ),
                ix_for(&program_id, "authorize_session_key", &[]),
            ],
        );

        validate_cosignable_tx(
            &tx,
            &program_id,
            &["create_game", "authorize_session_key"],
            &[game_pda],
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))
        .context("compute budget metadata must not block setup activation")?;
        Ok(())
    }

    #[test]
    fn rejects_a_smuggled_system_transfer() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();
        let attacker = Pubkey::new_unique();
        let tx = tx_of(
            &payer,
            &[
                ix_for(&program_id, "create_game", &[]),
                system_instruction::transfer(&payer.pubkey(), &attacker, 10_000_000),
            ],
        );

        let err = validate_cosignable_tx(&tx, &program_id, &["create_game"], &[])
            .err()
            .context("a System transfer must never be co-signed")?;
        assert!(err.contains("targets program"), "unexpected reason: {err}");
        Ok(())
    }

    #[test]
    fn rejects_an_unlisted_instruction_on_our_own_program() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();
        let tx = tx_of(&payer, &[ix_for(&program_id, "finalize_game", &[])]);

        let err = validate_cosignable_tx(&tx, &program_id, &["create_game"], &[])
            .err()
            .context("only the named instructions may be co-signed")?;
        assert!(err.contains("not one of"), "unexpected reason: {err}");
        Ok(())
    }

    #[test]
    fn rejects_a_transaction_for_a_different_game() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();
        let other_game_pda = Pubkey::new_unique();
        let expected_game_pda = Pubkey::new_unique();
        let tx = tx_of(
            &payer,
            &[ix_for(
                &program_id,
                "create_game",
                &[AccountMeta::new(other_game_pda, false)],
            )],
        );

        let err = validate_cosignable_tx(&tx, &program_id, &["create_game"], &[expected_game_pda])
            .err()
            .context("a tx for another game must be rejected")?;
        assert!(
            err.contains("does not reference"),
            "unexpected reason: {err}"
        );
        Ok(())
    }

    #[test]
    fn rejects_empty_and_oversized_transactions() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();

        let empty = tx_of(&payer, &[]);
        assert!(validate_cosignable_tx(&empty, &program_id, &["create_game"], &[]).is_err());

        let many: Vec<Instruction> = (0..MAX_INSTRUCTIONS + 1)
            .map(|_| ix_for(&program_id, "create_game", &[]))
            .collect();
        let oversized = tx_of(&payer, &many);
        let err = validate_cosignable_tx(&oversized, &program_id, &["create_game"], &[])
            .err()
            .context("instruction count must be bounded")?;
        assert!(err.contains("at most"), "unexpected reason: {err}");
        Ok(())
    }

    #[test]
    fn rejects_truncated_instruction_data() -> anyhow::Result<()> {
        let program_id = program();
        let payer = Keypair::new();
        let stub = Instruction {
            program_id,
            accounts: vec![],
            data: vec![1, 2, 3],
        };
        let tx = tx_of(&payer, &[stub]);

        let err = validate_cosignable_tx(&tx, &program_id, &["create_game"], &[])
            .err()
            .context("data shorter than a discriminator must be rejected")?;
        assert!(err.contains("too short"), "unexpected reason: {err}");
        Ok(())
    }
}
