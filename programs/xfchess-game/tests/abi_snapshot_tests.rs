//! P2: lock the public instruction ABI. The checked-in IDL is rendered to a
//! stable text form (discriminator, account metas in order, argument types) and
//! compared with `tests/snapshots/abi.txt`. Any change to an instruction's
//! name, discriminator, account order/flags or arguments fails this test until
//! the snapshot is deliberately regenerated:
//!
//!     UPDATE_ABI_SNAPSHOT=1 cargo test -p xfchess-game --test abi_snapshot_tests
//!
//! Account sizes of the persistent state types are part of the snapshot too.
use anchor_lang::Space;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn idl_path() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // XFChess monorepo layout first, then the extracted contracts repository.
    [
        root.join("../../xfchessdotcom/src/lib/xfchess_game.json"),
        root.join("../../idl/xfchess_game.json"),
    ]
    .into_iter()
    .find(|path| path.exists())
    .unwrap_or_else(|| root.join("../../idl/xfchess_game.json"))
}

fn type_name(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(name) => name.clone(),
        serde_json::Value::Object(map) => {
            if let Some(inner) = map.get("option") {
                format!("Option<{}>", type_name(inner))
            } else if let Some(inner) = map.get("vec") {
                format!("Vec<{}>", type_name(inner))
            } else if let Some(array) = map.get("array").and_then(|a| a.as_array()) {
                format!("[{}; {}]", type_name(&array[0]), array[1])
            } else if let Some(defined) = map.get("defined") {
                defined
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("?")
                    .to_string()
            } else {
                value.to_string()
            }
        }
        other => other.to_string(),
    }
}

fn flatten_accounts(accounts: &[serde_json::Value], out: &mut Vec<String>) {
    for account in accounts {
        if let Some(nested) = account.get("accounts").and_then(|a| a.as_array()) {
            flatten_accounts(nested, out);
            continue;
        }
        let flag = |key: &str, mark: &'static str| -> &'static str {
            if account.get(key).and_then(|v| v.as_bool()).unwrap_or(false) {
                mark
            } else {
                ""
            }
        };
        out.push(format!(
            "{}{}{}{}",
            account["name"].as_str().unwrap_or("?"),
            flag("writable", ":w"),
            flag("signer", ":s"),
            flag("optional", ":o"),
        ));
    }
}

fn render() -> anyhow::Result<String> {
    let idl: serde_json::Value = serde_json::from_slice(&std::fs::read(idl_path())?)?;
    let mut out = String::new();
    writeln!(out, "program {}", idl["address"].as_str().unwrap_or("?"))?;
    let instructions = idl["instructions"].as_array().cloned().unwrap_or_default();
    writeln!(out, "instructions {}", instructions.len())?;
    for ix in &instructions {
        let discriminator: Vec<String> = ix["discriminator"]
            .as_array()
            .map(|bytes| {
                bytes
                    .iter()
                    .map(|b| format!("{:02x}", b.as_u64().unwrap_or(0)))
                    .collect()
            })
            .unwrap_or_default();
        let mut accounts = Vec::new();
        flatten_accounts(
            ix["accounts"].as_array().map_or(&[][..], |a| &a[..]),
            &mut accounts,
        );
        let args: Vec<String> = ix["args"]
            .as_array()
            .map(|args| {
                args.iter()
                    .map(|arg| {
                        format!(
                            "{}:{}",
                            arg["name"].as_str().unwrap_or("?"),
                            type_name(&arg["type"])
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        writeln!(
            out,
            "{} {} accounts=[{}] args=[{}]",
            ix["name"].as_str().unwrap_or("?"),
            discriminator.join(""),
            accounts.join(","),
            args.join(",")
        )?;
    }
    writeln!(out, "account-sizes (without 8-byte discriminator)")?;
    writeln!(out, "Game {}", xfchess_game::state::Game::INIT_SPACE)?;
    writeln!(
        out,
        "PlayerProfile {}",
        xfchess_game::state::PlayerProfile::INIT_SPACE
    )?;
    writeln!(
        out,
        "RecoveryIdentity {}",
        xfchess_game::state::RecoveryIdentity::INIT_SPACE
    )?;
    writeln!(
        out,
        "GlobalSessionDelegation {}",
        xfchess_game::state::GlobalSessionDelegation::INIT_SPACE
    )?;
    writeln!(
        out,
        "SessionDelegation {}",
        xfchess_game::state::SessionDelegation::INIT_SPACE
    )?;
    writeln!(
        out,
        "UsernameRecord {}",
        xfchess_game::state::UsernameRecord::LEN
    )?;
    Ok(out)
}

#[test]
fn the_instruction_abi_matches_the_checked_in_snapshot() -> anyhow::Result<()> {
    let rendered = render()?;
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/abi.txt");
    if std::env::var_os("UPDATE_ABI_SNAPSHOT").is_some() {
        std::fs::create_dir_all(snapshot.parent().unwrap_or(Path::new(".")))?;
        std::fs::write(&snapshot, &rendered)?;
        return Ok(());
    }
    let expected = std::fs::read_to_string(&snapshot)?.replace("\r\n", "\n");
    assert_eq!(
        rendered, expected,
        "ABI changed; review the diff, then regenerate with UPDATE_ABI_SNAPSHOT=1"
    );
    Ok(())
}

#[test]
fn every_idl_discriminator_is_the_anchor_derivation_of_its_name() -> anyhow::Result<()> {
    let idl: serde_json::Value = serde_json::from_slice(&std::fs::read(idl_path())?)?;
    let instructions = idl["instructions"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        instructions.len(),
        82,
        "instruction count is part of the ABI"
    );
    for ix in &instructions {
        let name = ix["name"].as_str().unwrap_or("?");
        let expected = Sha256::digest(format!("global:{name}").as_bytes());
        let actual: Vec<u8> = ix["discriminator"]
            .as_array()
            .map(|b| b.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect())
            .unwrap_or_default();
        assert_eq!(&actual[..], &expected[..8], "{name}");
    }
    Ok(())
}

#[test]
fn compiled_discriminators_agree_with_the_idl_for_representative_instructions() -> anyhow::Result<()>
{
    use anchor_lang::Discriminator;
    let idl: serde_json::Value = serde_json::from_slice(&std::fs::read(idl_path())?)?;
    let discriminator = |name: &str| -> Vec<u8> {
        idl["instructions"]
            .as_array()
            .and_then(|all| all.iter().find(|ix| ix["name"] == name))
            .and_then(|ix| ix["discriminator"].as_array())
            .map(|b| b.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect())
            .unwrap_or_default()
    };
    assert_eq!(
        discriminator("finalize_game"),
        xfchess_game::instruction::FinalizeGame::DISCRIMINATOR
    );
    assert_eq!(
        discriminator("record_move"),
        xfchess_game::instruction::RecordMove::DISCRIMINATOR
    );
    assert_eq!(
        discriminator("force_undelegate_after_timeout"),
        xfchess_game::instruction::ForceUndelegateAfterTimeout::DISCRIMINATOR
    );
    assert_eq!(
        discriminator("recover_stuck_delegation_v2"),
        xfchess_game::instruction::RecoverStuckDelegationV2::DISCRIMINATOR
    );
    Ok(())
}
