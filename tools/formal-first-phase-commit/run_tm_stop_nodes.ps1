param([Parameter(Mandatory)][string]$ZigExe)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$zig=(Resolve-Path -LiteralPath $ZigExe).Path
if ($zig -match '["%!\r\n]') { throw 'Unsupported Zig path' }
$token=[guid]::NewGuid().ToString('N').Substring(0,12)
$network="seata-stop-$token"
$image='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
$build=Join-Path (Split-Path $repo -Parent) '.seata-node-linux'
$stage=Join-Path $build "fixture-$token"
$evidence=Join-Path $repo "docs/phase-2/evidence/tm-stop-nodes/run-$token"
$containers=@()
$networkCreated=$false

function Get-OwnedNode([string]$Name) {
    $node=docker inspect $Name | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $node.Config.Labels.'seata.stop.fixture' -ne $token) {
        throw "Missing owned fixture container $Name"
    }
    $node
}

function Get-TestArtifact([string[]]$TargetArgs) {
    $messages=cargo +stable test -p seata-tm --test stop_attestation_tls @TargetArgs --no-run --offline --locked --message-format=json
    if($LASTEXITCODE -ne 0){throw 'Stop attestation fixture build failed'}
    $executables=@(foreach($line in $messages) {
        $row=$line | ConvertFrom-Json
        if($row.reason -eq 'compiler-artifact' -and $row.target.name -eq 'stop_attestation_tls' -and $row.executable) {$row.executable}
    })
    if($executables.Count -ne 1){throw 'Expected one stop attestation test executable'}
    $executables[0]
}

Push-Location $repo
try {
    $state=Join-Path $stage 'state'
    if (-not (Test-Path -LiteralPath $build)) { New-Item -ItemType Directory -Path $build | Out-Null }
    New-Item -ItemType Directory -Path $stage,$state,$evidence | Out-Null
    $cc=Join-Path $build 'zig-linux.cmd'
    $ar=Join-Path $build 'zig-ar.cmd'
    Set-Content -LiteralPath $cc -Encoding ascii -Value @('@echo off',('"{0}" cc -target x86_64-linux-gnu.2.31 %*' -f $zig))
    Set-Content -LiteralPath $ar -Encoding ascii -Value @('@echo off',('"{0}" ar %*' -f $zig))
    $env:CC_x86_64_unknown_linux_gnu=$cc
    $env:AR_x86_64_unknown_linux_gnu=$ar
    $env:CRATE_CC_NO_DEFAULTS='1'
    $env:CFLAGS_x86_64_unknown_linux_gnu='-fPIC'
    $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$cc
    $env:ZIG_GLOBAL_CACHE_DIR=Join-Path $build 'zig-global'
    $env:ZIG_LOCAL_CACHE_DIR=Join-Path $build 'zig-local'
    $env:CARGO_TARGET_DIR=Join-Path $build 'target'
    $env:CARGO_PROFILE_TEST_DEBUG='0'
    $env:CARGO_INCREMENTAL='0'
    $env:SEATA_TM_NODE_PROVISION_DIR=$stage
    $windows=Get-TestArtifact
    & $windows --exact --ignored node::provision
    if($LASTEXITCODE -ne 0){throw 'Test credential provisioning failed'}
    $linux=Get-TestArtifact @('--target','x86_64-unknown-linux-gnu')
    docker image inspect $image | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Pinned fixture image unavailable'}
    docker network create --internal --label "seata.stop.fixture=$token" $network | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Isolated fixture network creation failed'}
    $networkCreated=$true
    foreach($role in @('signer','recovery')) {
        $name="$network-$role"
        $creds=Join-Path $stage $role
        $publicState="type=bind,source=$state,target=/node-state"
        if($role -eq 'recovery'){$publicState+=',readonly'}
        $dockerArgs=@('run','-d','--pull=never','--read-only','--cap-drop=ALL',
            '--security-opt','no-new-privileges','--user','65534:65534',
            '--tmpfs','/var/lib/mysql:ro,noexec,nosuid,nodev',
            '--label',"seata.stop.fixture=$token",'--name',$name,'--network',$network,
            '--network-alias',"$role.node.test",'--mount',"type=bind,source=$linux,target=/stop-node,readonly",
            '--mount',"type=bind,source=$creds,target=/node-credentials,readonly",
            '--mount',$publicState,'-e','SEATA_TM_NODE_CREDENTIAL_DIR=/node-credentials',
            '-e','SEATA_TM_NODE_STATE_DIR=/node-state','--entrypoint','/stop-node',
            $image,'--exact','--ignored','--nocapture',"node::$role")
        docker @dockerArgs | Out-Null
        if($LASTEXITCODE -ne 0){throw "Cannot start $role node"}
        $containers+=@($name)
        if($role -eq 'signer'){
            for($i=0; $i -lt 80 -and -not (Test-Path -LiteralPath (Join-Path $state 'ready')); $i++){
                if(-not (Get-OwnedNode $name).State.Running){throw 'Signer exited before readiness'}
                Start-Sleep -Milliseconds 100
            }
            if(-not (Test-Path -LiteralPath (Join-Path $state 'ready'))){throw 'Signer readiness timed out'}
        }
    }
    $nodes=@{}
    foreach($role in @('signer','recovery')){
        $name="$network-$role"
        for($i=0; $i -lt 160; $i++){
            $node=Get-OwnedNode $name
            if(-not $node.State.Running){break}
            Start-Sleep -Milliseconds 100
        }
        if($node.State.Running -or $node.State.ExitCode -ne 0){throw "$role did not succeed"}
        $credentialMount=@($node.Mounts | Where-Object Destination -eq '/node-credentials')
        $stateMount=@($node.Mounts | Where-Object Destination -eq '/node-state')
        $binds=@($node.Mounts | Where-Object Type -eq 'bind')
        $published=@($node.HostConfig.PortBindings.PSObject.Properties | Where-Object Value)
        if($binds.Count -ne 3 -or $credentialMount.Count -ne 1 -or $credentialMount[0].RW -or
            $stateMount.Count -ne 1 -or $stateMount[0].RW -ne ($role -eq 'signer') -or
            $node.HostConfig.NetworkMode -ne $network -or $published.Count -ne 0){
            throw "Unexpected fixture isolation for ${role}: binds=$($binds.Count) credentials=$($credentialMount.Count) state=$($stateMount.Count) network=$($node.HostConfig.NetworkMode) ports=$($published.Count)"
        }
        $nodes[$role]=@{id=$node.Id; exit_code=$node.State.ExitCode; credentials_read_only=$true;
            public_state_read_only=(-not $stateMount[0].RW); published_ports=0}
    }
    $link=docker logs "$network-signer"
    if($LASTEXITCODE -ne 0 -or -not (($link -join [Environment]::NewLine) -match 'NODE_LINK signer=([0-9.]+) recovery=([0-9.]+)')){
        throw 'Missing TCP peer addresses from isolated nodes'
    }
    $nodes.signer.ip=$Matches[1]
    $nodes.recovery.ip=$Matches[2]
    if($nodes.signer.ip -eq $nodes.recovery.ip){throw 'Expected two distinct network addresses'}
    $result=docker logs "$network-recovery"
    if($LASTEXITCODE -ne 0 -or (($result -join [Environment]::NewLine) -notmatch 'NODE_STOP_PROOF old_accepted=1 new_accepted=2 retired_rejected=1')){
        throw 'Missing recovery node verification record'
    }
    @{nodes=$nodes; image=$image; rustc=(& rustc +stable --version); zig=(& $zig version);
        linux_sha256=(Get-FileHash -LiteralPath $linux -Algorithm SHA256).Hash;
        tcp_proofs_verified=3; retired_signature_rejected=1; tls='1.3'; isolated_network=$network
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidence 'verified.json')
    Write-Output "PASS isolated TM stop nodes: $evidence"
} finally {
    try {
        foreach($container in $containers){
            [void](Get-OwnedNode $container)
            docker logs $container 2>&1 | Set-Content -LiteralPath (Join-Path $evidence "$container.log")
            docker rm -fv $container | Out-Null
            if($LASTEXITCODE -ne 0){throw "Fixture cleanup failed for $container"}
        }
        if($networkCreated){
            docker network rm $network | Out-Null
            if($LASTEXITCODE -ne 0){throw 'Fixture network cleanup failed'}
        }
    } finally {
        if(Test-Path -LiteralPath $stage) {
            $actual=(Resolve-Path -LiteralPath $stage).Path
            $expected=Join-Path (Resolve-Path -LiteralPath $build).Path "fixture-$token"
            if($actual -ne $expected){throw 'Unexpected fixture credential path'}
            Remove-Item -LiteralPath $actual -Recurse -Force
        }
        Pop-Location
    }
    @{containers_removed=$containers.Count; network_removed=$networkCreated;
        credential_directory_removed=(-not (Test-Path -LiteralPath $stage))
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'cleanup.json')
}
