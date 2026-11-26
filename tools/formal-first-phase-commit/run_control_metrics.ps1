param([switch]$Mutation)
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Set-Location -LiteralPath $repoRoot
$container='seata-control-metrics-'+[guid]::NewGuid().ToString('N').Substring(0,12)
$image='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
$created=$false
try {
    docker image inspect $image --format '{{.Id}}' | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Pinned MySQL image missing'}
    docker run -d --rm --pull=never --name $container --label seata.fixture.owner=mysql-control-metrics -p 127.0.0.1::3306 -e MYSQL_ROOT_PASSWORD=seata-mysql-fixture-only -e MYSQL_ROOT_HOST=% $image | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Owned container failed'}
    $created=$true
    $ready=$false
    for($attempt=0;$attempt -lt 45;$attempt++){
        docker exec $container mysql --user=root --password=seata-mysql-fixture-only --batch --skip-column-names -e 'SELECT 1' 2>$null | Out-Null
        if($LASTEXITCODE -eq 0){
            # Require the published host handshake as well as the local bootstrap server.
            $binding=docker port $container 3306/tcp
            $port=[int]($binding -split ':')[-1]
            try {
                $client=[System.Net.Sockets.TcpClient]::new()
                $client.Connect('127.0.0.1',$port)
                $client.ReceiveTimeout=1000
                $header=New-Object byte[] 4
                $n=$client.GetStream().Read($header,0,4)
                $client.Dispose()
                if($n -eq 4 -and $header[3] -eq 0){$ready=$true;break}
            }catch{if($null -ne $client){$client.Dispose()}}
        }
        Start-Sleep -Seconds 2
    }
    if(-not $ready){throw 'Owned MySQL TCP handshake timed out'}
    $env:SEATA_MYSQL_PORT="$port"
    $env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
    $env:SEATA_MYSQL_DATABASE='undo_control_metrics'
    $work=Join-Path 'D:/codex-builds' $container
    New-Item -ItemType Directory -Path $work | Out-Null
    $env:SEATA_UNDO_SCHEMA_PATH=Join-Path $work 'client-schema.sql'
    $env:SEATA_UNDO_STORE_OUT=$work
    python -c "import subprocess,pathlib,sys,hashlib;raw=subprocess.check_output(['git','-C','../seata-java-phase09-oracle','show','cba73e180fd8896a99df6415b859d275122feaa7:script/client/at/db/mysql.sql']);assert hashlib.sha256(raw).hexdigest()=='8dcd994e28d31d494b0d3f90663c74d788fdb7250ef6ce0b8267b66170e91ce9';pathlib.Path(sys.argv[1]).write_bytes(raw)" $env:SEATA_UNDO_SCHEMA_PATH
    if($LASTEXITCODE -ne 0){throw 'Pinned client schema failed'}
    docker exec $container mysql --user=root --password=seata-mysql-fixture-only -e 'CREATE DATABASE undo_control_metrics'
    if($LASTEXITCODE -ne 0){throw 'Owned database failed'}
    $env:TEMP='D:/Temp';$env:TMP='D:/Temp'
    $env:CARGO_HOME=(Resolve-Path '../.seata-cargo-cache').Path
    $env:CARGO_PROFILE_TEST_DEBUG='0';$env:CARGO_INCREMENTAL='0'
    $runtime=([ordered]@{image=$image;container=$container;port=$port;database=$env:SEATA_MYSQL_DATABASE;schema_sha256='8dcd994e28d31d494b0d3f90663c74d788fdb7250ef6ce0b8267b66170e91ce9';mysql_version=(docker exec $container mysql --user=root --password=seata-mysql-fixture-only --batch --skip-column-names -e 'SELECT VERSION()')})
    if($LASTEXITCODE -ne 0){throw 'Runtime version check failed'}
    $prefix=if($Mutation){'mutation'}else{'live'}
    $runtime | ConvertTo-Json | Set-Content -Encoding utf8 "docs/phase-2/evidence/mysql-control-metrics/$prefix-runtime.json"
    if($Mutation){
        $env:CARGO_TARGET_DIR=(Resolve-Path '../.seata-target-nodebug').Path
        python -B tools/formal-first-phase-commit/control_metrics_mutation.py
        if($LASTEXITCODE -ne 0){throw 'Mutation gate failed'}
    }else{
        foreach($version in @('1.99','1.85')){
            $env:CARGO_TARGET_DIR=(Resolve-Path $(if($version -eq '1.99'){'../.seata-target-nodebug'}else{'../.seata-target-nodebug-185'})).Path
            & cargo "+$version.0" test -p seata-db-mysql --test live_recovery_metrics --all-features --offline --locked -- --ignored --test-threads=1 *> "docs/phase-2/evidence/mysql-control-metrics/$version-live.log"
            if($LASTEXITCODE -ne 0){Get-Content "docs/phase-2/evidence/mysql-control-metrics/$version-live.log" -Tail 35;throw 'Live gate failed'}
            Get-Content "docs/phase-2/evidence/mysql-control-metrics/$version-live.log" -Tail 5
            & cargo "+$version.0" test -p seata-observability --features mysql --test mysql_recovery --offline --locked -- --ignored *> "docs/phase-2/evidence/mysql-control-metrics/$version-http.log"
            if($LASTEXITCODE -ne 0){Get-Content "docs/phase-2/evidence/mysql-control-metrics/$version-http.log" -Tail 30;throw 'HTTP gate failed'}
        }
    }
}finally{
    if($created){
        docker stop --time 5 $container | Out-Null
        if($LASTEXITCODE -ne 0){throw 'Owned container cleanup failed'}
        $remaining=docker ps -a --filter "name=^/$container$" --format '{{.Names}}'
        if($LASTEXITCODE -ne 0 -or $remaining){throw 'Owned container still present'}
        $prefix=if($Mutation){'mutation'}else{'live'}
        @{container=$container;removed=$true} | ConvertTo-Json | Set-Content -Encoding utf8 "docs/phase-2/evidence/mysql-control-metrics/$prefix-cleanup.json"
    }
}
