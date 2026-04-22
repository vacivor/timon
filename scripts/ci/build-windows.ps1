$ErrorActionPreference = "Stop"

$binaryName = if ($env:BINARY_NAME) { $env:BINARY_NAME } else { "timon" }
$profile = if ($env:PROFILE) { $env:PROFILE } else { "release" }
$targetTriple = if ($env:TARGET_TRIPLE) {
  $env:TARGET_TRIPLE
} else {
  (rustc -vV | Select-String '^host: ' | ForEach-Object {
    $_.ToString().Replace('host: ', '').Trim()
  })
}
$distDir = if ($env:DIST_DIR) { $env:DIST_DIR } else { "dist" }
$archiveBaseName = if ($env:ARCHIVE_BASENAME) {
  $env:ARCHIVE_BASENAME
} else {
  "$binaryName-$targetTriple"
}

cargo build --locked --profile $profile --target $targetTriple

$buildDir = Join-Path "target" "$targetTriple/$profile"
$stageDir = Join-Path $distDir $archiveBaseName

if (Test-Path $stageDir) {
  Remove-Item $stageDir -Recurse -Force
}

New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
Copy-Item (Join-Path $buildDir "$binaryName.exe") $stageDir

$archivePath = Join-Path $distDir "$archiveBaseName.zip"
if (Test-Path $archivePath) {
  Remove-Item $archivePath -Force
}

Compress-Archive -Path (Join-Path $stageDir '*') -DestinationPath $archivePath
