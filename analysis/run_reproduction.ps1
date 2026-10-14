param(
    [Parameter(Mandatory=$true)][string]$Out,
    [string]$Executable='target\delivery\release\genesis.exe',
    [string]$Spec='experiments\matter-v2.json',
    [string]$Protocol='experiments/matter-reproduction-protocol.json',
    [UInt64[]]$DiscoverySeeds=@(101,102,103),
    [UInt64[]]$ConfirmationSeeds=@(104,105,106)
)
$ErrorActionPreference='Stop'
$owned=$false
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $outputPath=[IO.Path]::GetFullPath($Out)
    $binary=[IO.Path]::GetFullPath($Executable)
    if(Test-Path -LiteralPath $outputPath){throw 'Output directory must be new.'}
    if(-not (Test-Path -LiteralPath $binary -PathType Leaf)){throw 'Build the release executable first.'}
    $allSeeds=@($DiscoverySeeds)+@($ConfirmationSeeds)
    if($DiscoverySeeds.Count -eq 0 -or $ConfirmationSeeds.Count -eq 0 -or @($allSeeds|Select-Object -Unique).Count -ne $allSeeds.Count){throw 'Nonempty, distinct discovery and confirmation seeds are required.'}
    New-Item -ItemType Directory $outputPath|Out-Null
    $owned=$true
    $inputs=Join-Path $outputPath 'inputs'
    New-Item -ItemType Directory $inputs|Out-Null
    Copy-Item -LiteralPath $binary -Destination (Join-Path $inputs 'genesis.exe')
    Copy-Item -LiteralPath $Spec -Destination (Join-Path $inputs 'spec.json')
    Copy-Item -LiteralPath $Protocol -Destination (Join-Path $inputs 'protocol.json')
    $binary=Join-Path $inputs 'genesis.exe'
    $specPath=Join-Path $inputs 'spec.json'
    $protocolPath=Join-Path $inputs 'protocol.json'
    @{binary_sha256=(Get-FileHash $binary).Hash;spec_sha256=(Get-FileHash $specPath).Hash;protocol_sha256=(Get-FileHash $protocolPath).Hash;
        discovery_seeds=$DiscoverySeeds;confirmation_seeds=$ConfirmationSeeds;claim='Controlled investigation of autonomous matching assembly, maintenance and repeated production; no claim of life.'}|
        ConvertTo-Json -Depth 5|Set-Content (Join-Path $outputPath 'workflow.json')
    foreach($phase in @('discovery','confirmation')){
        $seeds=if($phase -eq 'discovery'){$DiscoverySeeds}else{$ConfirmationSeeds}
        Write-Host $phase
        & $binary lab reproduction --spec $specPath --protocol $protocolPath --seeds ($seeds -join ',') --out (Join-Path $outputPath $phase) 1> (Join-Path $outputPath ($phase+'.stdout.log')) 2> (Join-Path $outputPath ($phase+'.stderr.log'))
        if($LASTEXITCODE -ne 0){throw ($phase+' failed; diagnostic artifacts are retained.')}
    }
    $discovery=Get-Content (Join-Path $outputPath 'discovery/reproduction.json') -Raw|ConvertFrom-Json
    $confirmation=Get-Content (Join-Path $outputPath 'confirmation/reproduction.json') -Raw|ConvertFrom-Json
    $eligibility=@{}
    foreach($phase in @('discovery','confirmation')){
        $report=if($phase -eq 'discovery'){$discovery}else{$confirmation}
        $eligibility[$phase]=if($null -ne $report.PSObject.Properties['assayed_world_seeds']){
            @{status='recorded';assayed_worlds=@($report.assayed_world_seeds);ineligible_worlds=@($report.ineligible_world_seeds)}
        }else{@{status='not-recorded-by-this-executable'}}
    }
    @{completed=$true;discovery_supported_worlds=@($discovery.supported_world_seeds);confirmation_supported_worlds=@($confirmation.supported_world_seeds);
        eligibility=$eligibility;
        heritable_evolution_demonstrated=$false;interpretation='Read maintenance, production, enrichment and descendant controls separately. Matching structures alone do not establish heredity.'}|
        ConvertTo-Json -Depth 5|Set-Content (Join-Path $outputPath 'workflow-result.json')
    Write-Host ('Completed both phases: '+$outputPath)
} catch {
    if($owned){@{completed=$false;phase=$phase;error=$_.Exception.Message}|ConvertTo-Json|Set-Content (Join-Path $outputPath 'workflow-failure.json')}
    throw
} finally {Pop-Location}
