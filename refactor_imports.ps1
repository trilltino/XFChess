$mappings = @{
    'multiplayer::\bbraid_node\b' = 'multiplayer::network::braid'
    'multiplayer::\bp2p_connection\b' = 'multiplayer::network::p2p'
    'multiplayer::\bnetwork_protocol\b' = 'multiplayer::network::protocol'
    'multiplayer::\bsolana_integration\b' = 'multiplayer::solana::integration'
    'multiplayer::\bsolana_lobby_state\b' = 'multiplayer::solana::lobby'
    'multiplayer::\bsolana_addon\b' = 'multiplayer::solana::addon'
    'multiplayer::\btauri_signer\b' = 'multiplayer::solana::tauri_signer'
    'multiplayer::\bgame_id_store\b' = 'multiplayer::solana::game_id_store'
    'multiplayer::\brollup_manager\b' = 'multiplayer::rollup::manager'
    'multiplayer::\brollup_network_bridge\b' = 'multiplayer::rollup::bridge'
    'multiplayer::\bsession_key_manager\b' = 'multiplayer::rollup::session_keys'
    'multiplayer::\bvps_client\b' = 'multiplayer::rollup::vps_client'
    'multiplayer::\bmagicblock_resolver\b' = 'multiplayer::rollup::magicblock'
    'multiplayer::\bephemeral_mvp_plugin\b' = 'multiplayer::rollup::mvp_plugin'
    'multiplayer::\btransaction_debugger\b' = 'multiplayer::ui::tx_debugger'
}

Get-ChildItem -Path "src" -Recurse -File -Filter "*.rs" | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    $original = $content
    foreach ($key in $mappings.Keys) {
        $content = [System.Text.RegularExpressions.Regex]::Replace($content, $key, $mappings[$key])
    }
    if ($original -ne $content) {
        Set-Content -Path $_.FullName -Value $content -NoNewline
        Write-Host "Updated $($_.FullName)"
    }
}
