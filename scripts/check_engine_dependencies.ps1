$ErrorActionPreference = 'Stop'

foreach ($configuration in @(
    @('-p', 'xfchess'),
    @('-p', 'xfchess', '--features', 'solana'),
    @('-p', 'backend'),
    @('-p', 'cu_benchmark')
)) {
    $cargoArguments = @('tree', '--locked', '--edges', 'normal,build', '--prefix', 'none') + $configuration
    $dependencyTree = & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) { throw "cargo tree failed: $configuration" }
    if ($dependencyTree -match 'nimzovich_engine') {
        throw "Nimzovich leaked into application dependencies: $configuration"
    }
    Write-Output "Engine boundary OK: $configuration"
}

$contractTree = & cargo tree --locked -p xfchess-game --edges features -i nimzovich_engine
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect contract engine features' }
if ($contractTree -match 'nimzovich_engine feature "search"') {
    throw 'Contract dependencies must not enable engine search'
}
Write-Output 'Contract rules retain Nimzovich with search disabled.'
