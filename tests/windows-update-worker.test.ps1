$ErrorActionPreference='Stop'
$repository=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$root=Join-Path ([IO.Path]::GetTempPath()) ('ontime-update-qa-'+[Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $root | Out-Null
foreach($version in @('0.0.1-beta5','0.0.1-beta6')) {
 $source='using System;using System.IO;using System.Reflection;[assembly:AssemblyVersion("1.0.0.0")][assembly:AssemblyInformationalVersion("'+$version+'") ]class Program{static void Main(){File.WriteAllText(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"restarted.txt"),"'+$version+'");}}'
 $source | Set-Content -LiteralPath "$root\$version.cs" -Encoding UTF8
 & "$env:WINDIR\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:winexe "/out:$root\$version.exe" "$root\$version.cs"
 if($LASTEXITCODE -ne 0){throw 'Compiler failed'}
}
foreach($mode in @('success','quick-exit','failed','tampered')) {
 $case=Join-Path $root $mode
 $install=Join-Path $case 'Program with spaces & 更新'
 New-Item -ItemType Directory -Force -Path $install | Out-Null
 Copy-Item -LiteralPath "$root\0.0.1-beta5.exe" -Destination "$install\ontime.exe" -Force
 [IO.File]::WriteAllText("$install\data.db",'original-reminder-data')
 $installer="$case\onTime_test_x64-setup.exe"
 $body=if($mode -eq 'failed'){'Abort "Simulated failure"'}else{'File /oname=ontime.exe "'+$root+'\0.0.1-beta6.exe"'}
 $nsi=@('Unicode true','Name "onTime isolated fixture"',('OutFile "'+$installer+'"'),'RequestExecutionLevel user','SilentInstall silent','!include "FileFunc.nsh"','Section','${GetParameters} $R0','${GetOptions} $R0 "/UPDATE" $R1','IfErrors 0 +2','Abort "Update mode required"','SetOutPath "$INSTDIR"','FileOpen $0 "$INSTDIR\installer-started.txt" w','FileWrite $0 "started"','FileClose $0',$body,'SectionEnd') -join [Environment]::NewLine
 [IO.File]::WriteAllText("$case\fixture.nsi",$nsi)
 & (Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe') /V1 "$case\fixture.nsi"
 if($LASTEXITCODE -ne 0){throw 'NSIS fixture failed'}
 if($mode -eq 'quick-exit'){
  $parentScript=Join-Path $case 'parent.ps1'
  [IO.File]::WriteAllText($parentScript, "while (-not (Test-Path -LiteralPath '$case\worker.ready')) { Start-Sleep -Milliseconds 10 }")
  $parentArgs='-NoProfile -NonInteractive -File "'+$parentScript+'"'
 }else{ $parentArgs='-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 5"' }
 $parent=Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList $parentArgs -WindowStyle Hidden -PassThru
 $plan=@{installer=$installer;executable="$install\ontime.exe";installDir=$install;sha256=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash;version='0.0.1-beta6';parentPid=$parent.Id;ready="$case\worker.ready";receipt="$case\result.json"}
 if($mode -eq 'tampered'){$plan.sha256='0'*64}
 [IO.File]::WriteAllText("$case\plan.json",($plan|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
 $script=Join-Path $repository 'src-tauri\src\update\install-worker.ps1'
 $workerArgs='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+$script+'" -PlanPath "'+$case+'\plan.json"'
 $worker=Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList $workerArgs -WindowStyle Hidden -PassThru
 if($mode -ne 'tampered'){
  $limit=[DateTime]::UtcNow.AddSeconds(4)
  while(-not(Test-Path -LiteralPath $plan.ready) -and [DateTime]::UtcNow -lt $limit){Start-Sleep -Milliseconds 50}
  if(-not(Test-Path -LiteralPath $plan.ready)){throw 'Handshake failed'}
  if($mode -ne 'quick-exit' -and (Test-Path -LiteralPath "$install\installer-started.txt")){throw 'Installer started before app exit'}
 }
 $parent.WaitForExit()
 if(-not $worker.WaitForExit(30000)){throw 'Worker timeout'}
 $result=Get-Content -LiteralPath $plan.receipt -Raw -Encoding UTF8|ConvertFrom-Json
 if((Get-Content -LiteralPath "$install\data.db" -Raw) -ne 'original-reminder-data'){throw 'Data changed'}
 if($mode -eq 'tampered'){
  if($result.status -ne 'failed' -or (Test-Path -LiteralPath $plan.ready) -or (Test-Path -LiteralPath "$install\installer-started.txt")){throw 'Tamper accepted'}
 }else{
  $expected=if($mode -in @('success','quick-exit')){'0.0.1-beta6'}else{'0.0.1-beta5'}
  $expectedStatus=if($mode -eq 'quick-exit'){'success'}else{$mode}
  if($result.status -ne $expectedStatus){throw ($result|ConvertTo-Json)}
  $limit=[DateTime]::UtcNow.AddSeconds(3)
  while(-not(Test-Path -LiteralPath "$install\restarted.txt") -and [DateTime]::UtcNow -lt $limit){Start-Sleep -Milliseconds 50}
  if((Get-Content -LiteralPath "$install\restarted.txt" -Raw) -ne $expected){throw 'Expected app did not restart'}
 }
 Write-Output ('PASS '+$mode+': parent exit ordering, NSIS installation/restart or recovery, checksum, original data')
}
