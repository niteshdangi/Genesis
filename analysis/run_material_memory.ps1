param(
    [Parameter(Mandatory=$true)][string]$Out,
    [string]$Executable = 'target/validation/release/genesis.exe',
    [string]$Spec = 'experiments/matter-shape-memory-spec.json',
    [string]$Protocol = 'experiments/matter-trimer-memory-protocol.json',
    [UInt64[]]$DiscoverySeeds = @(22,23,24),
    [UInt64[]]$ConfirmationSeeds = @(25,26,27),
    [ValidateRange(32,64)][int]$Replicas = 64
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$ownsOutput = $false
Push-Location $workspace
try {
    $outputPath = [IO.Path]::GetFullPath($Out)
    $enginePath = [IO.Path]::GetFullPath($Executable)
    if (-not (Test-Path -LiteralPath $enginePath -PathType Leaf)) { throw 'Build the release executable before starting this workflow.' }
    if (Test-Path -LiteralPath $outputPath) { throw 'The output directory must be new.' }
    if ($DiscoverySeeds.Count -eq 0 -or $ConfirmationSeeds.Count -eq 0) { throw 'Both seed sets are required.' }
    if (@($DiscoverySeeds | Where-Object { $ConfirmationSeeds -contains $_ }).Count -gt 0) { throw 'Discovery and confirmation worlds must be distinct.' }
    if (@($DiscoverySeeds | Select-Object -Unique).Count -ne $DiscoverySeeds.Count -or @($ConfirmationSeeds | Select-Object -Unique).Count -ne $ConfirmationSeeds.Count) { throw 'World seeds must be unique within each phase.' }
    New-Item -ItemType Directory -Path $outputPath | Out-Null
    $ownsOutput = $true
    $stage = 'prepare'
    $inputs = Join-Path $outputPath 'inputs'
    New-Item -ItemType Directory -Path $inputs | Out-Null
    Copy-Item -LiteralPath $Spec -Destination (Join-Path $inputs 'spec.json')
    Copy-Item -LiteralPath $Protocol -Destination (Join-Path $inputs 'protocol.json')
    Copy-Item -LiteralPath $enginePath -Destination (Join-Path $inputs 'genesis.exe')
    $Spec = Join-Path $inputs 'spec.json'
    $Protocol = Join-Path $inputs 'protocol.json'
    $enginePath = Join-Path $inputs 'genesis.exe'
    @{
        executable_sha256 = (Get-FileHash -LiteralPath $enginePath -Algorithm SHA256).Hash
        spec_sha256 = (Get-FileHash -LiteralPath $Spec -Algorithm SHA256).Hash
        protocol_sha256 = (Get-FileHash -LiteralPath $Protocol -Algorithm SHA256).Hash
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $inputs 'sha256.json') -Encoding utf8
    function Invoke-Stage([string]$Name, [string[]]$EngineArguments) {
        Write-Host $Name
        & $enginePath @EngineArguments 1> (Join-Path $outputPath ($Name + '.stdout.log')) 2> (Join-Path $outputPath ($Name + '.stderr.log'))
        if ($LASTEXITCODE -ne 0) { throw "$Name failed; its artifacts and diagnostic log are retained." }
    }
    @{
        spec = $Spec; protocol = $Protocol; discovery_seeds = $DiscoverySeeds; confirmation_seeds = $ConfirmationSeeds
        replicas = $Replicas; hypothesis = 'Stored spatial arrangement changes later energy transmission.'
        claim_limit = 'Exploratory causal response experiment, not adaptive memory, life, or a scientific novelty claim.'
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputPath 'workflow.json') -Encoding utf8
    foreach ($phase in @('discovery','confirmation')) {
        $stage = $phase
        $seeds = if ($phase -eq 'discovery') { $DiscoverySeeds } else { $ConfirmationSeeds }
        $conditioning = Join-Path $outputPath ($phase + '-conditioning')
        $audit = Join-Path $outputPath ($phase + '-audit')
        Invoke-Stage ($phase + '-conditioning') @('lab','cluster-prepare','--spec',$Spec,'--protocol',$Protocol,'--seeds',($seeds -join ','),'--out',$conditioning)
        Invoke-Stage ($phase + '-audit') @('lab','geometry-audit','--source',$conditioning,'--out',$audit,'--replicas',"$Replicas")
    }
    $discovery = Get-Content -LiteralPath (Join-Path $outputPath 'discovery-audit/geometry-audit.json') -Raw | ConvertFrom-Json
    $confirmation = Get-Content -LiteralPath (Join-Path $outputPath 'confirmation-audit/geometry-audit.json') -Raw | ConvertFrom-Json
    @{
        completed = $true
        discovery_supported_worlds = @($discovery.supported_world_seeds)
        confirmation_supported_worlds = @($confirmation.supported_world_seeds)
        interpretation = 'Read the per-cluster controls and negative/ineligible cases; support flags are exploratory.'
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputPath 'workflow-result.json') -Encoding utf8
    Write-Host ('Completed both phases: ' + $outputPath)
} catch {
    if ($ownsOutput -and (Test-Path -LiteralPath $outputPath -PathType Container)) {
        @{ completed=$false; stage=$stage; error=$_.Exception.Message } | ConvertTo-Json |
            Set-Content -LiteralPath (Join-Path $outputPath 'workflow-failure.json') -Encoding utf8
    }
    throw
} finally {
    Pop-Location
}
