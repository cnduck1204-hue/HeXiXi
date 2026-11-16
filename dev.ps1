param(
    [ValidateSet('build', 'run', 'test', 'clean')]
    [string]$Action = 'run',
    [string]$Image = 'rcore-tutorial-v3:latest',
    [switch]$Offline
)

$ErrorActionPreference = 'Stop'
$dockerArguments = @('run', '--rm', '--platform', 'linux/amd64')
if ($Action -eq 'run') {
    $dockerArguments += '-it'
}
$dockerArguments += @(
    '--mount', 'type=volume,source=hexixi-cargo-registry,target=/usr/local/cargo/registry',
    '--mount', 'type=volume,source=hexixi-cargo-git,target=/usr/local/cargo/git',
    '--mount', "type=bind,source=$PSScriptRoot,target=/hexixi",
    '--workdir', '/hexixi'
)
if ($Offline) {
    $dockerArguments += @('--env', 'CARGO_NET_OFFLINE=true')
}
$dockerArguments += @(
    '--entrypoint', 'bash', $Image,
    '-lc', "make $Action"
)
& docker @dockerArguments
if ($LASTEXITCODE -ne 0) {
    throw "HeXiXi $Action failed (exit code $LASTEXITCODE)."
}
