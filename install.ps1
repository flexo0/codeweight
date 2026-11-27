# Installs codeweight's release binary on Windows:
#
#   powershell -ExecutionPolicy Bypass -c "irm https://github.com/flexo0/codeweight/releases/latest/download/install.ps1 | iex"
#
# CODEWEIGHT_VERSION picks the release (`0.2.0` or `v0.2.0`; the default: this script's own release, else the latest),
# CODEWEIGHT_INSTALL_DIR the folder (default `%LOCALAPPDATA%\Programs\codeweight`), which is added to the user's Path
# unless CODEWEIGHT_NO_MODIFY_PATH is set. The archive is checked against its `.sha256` before anything is installed.
# How it's released: docs/releasing.md.

# A script block, not the script's top level: `irm | iex` runs it in the caller's session, where `exit` would close
# the window and a variable would be left behind.
& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'

    # The release this copy is attached to, set by release.yml; empty in the repo, where the latest is fetched.
    $releaseTag = ''

    $repo = 'flexo0/codeweight'

    # Downloads a URL to a file; a failure says which URL and why, not the page GitHub answered with.
    function Get-Url($url, $file) {
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $file
        } catch {
            throw "codeweight: couldn't download ${url}: $($_.Exception.Message)"
        }
    }

    # Windows PowerShell 5.1 may default to TLS 1.0, which GitHub refuses.
    $tls12 = [Net.SecurityProtocolType]::Tls12
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor $tls12

    # The CPU of Windows itself, not of this PowerShell (a 32-bit one on 64-bit Windows says x86).
    $cpu = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    # Windows 11 on ARM runs the x86_64 binary under its emulation.
    if ($cpu -ne 'AMD64' -and $cpu -ne 'ARM64') {
        throw "codeweight: no release binary for Windows on $cpu; build it with ``cargo install codeweight``"
    }
    $target = 'x86_64-pc-windows-msvc'

    $tag = if ($env:CODEWEIGHT_VERSION) {
        if ($env:CODEWEIGHT_VERSION.StartsWith('v')) { $env:CODEWEIGHT_VERSION } else { "v$env:CODEWEIGHT_VERSION" }
    } elseif ($releaseTag) {
        $releaseTag
    } else {
        (Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$repo/releases/latest").tag_name
    }
    if (-not $tag) { throw "codeweight: couldn't find the latest release of $repo" }

    $dir = if ($env:CODEWEIGHT_INSTALL_DIR) { $env:CODEWEIGHT_INSTALL_DIR } else {
        Join-Path $env:LOCALAPPDATA 'Programs\codeweight'
    }
    $name = "codeweight-$tag-$target"
    $url = "https://github.com/$repo/releases/download/$tag/$name.zip"

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Write-Host "codeweight: downloading $name.zip"
        $zip = Join-Path $tmp "$name.zip"
        Get-Url $url $zip
        Get-Url "$url.sha256" "$zip.sha256"
        $expected = ((Get-Content "$zip.sha256" -Raw).Trim() -split '\s+')[0]
        $actual = (Get-FileHash -Algorithm SHA256 $zip).Hash
        if ($expected -ne $actual) {
            throw "codeweight: $name.zip's SHA-256 is $actual, not the release's $expected"
        }

        Expand-Archive -Path $zip -DestinationPath $tmp
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        Copy-Item -Force (Join-Path $tmp "$name\codeweight.exe") (Join-Path $dir 'codeweight.exe')
        Write-Host "codeweight: installed $tag to $(Join-Path $dir 'codeweight.exe')"
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $onPath = ($userPath -split ';') -contains $dir
    if (-not $onPath -and -not $env:CODEWEIGHT_NO_MODIFY_PATH) {
        $newPath = if ($userPath) { "$userPath;$dir" } else { $dir }
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        $env:Path = "$env:Path;$dir"
        Write-Host "codeweight: added $dir to your Path; a terminal opened before this needs restarting to see it"
    } elseif (-not $onPath) {
        Write-Host "codeweight: $dir isn't on your Path; add it to run codeweight by name"
    }
}
