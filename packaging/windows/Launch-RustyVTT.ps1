param([switch]$NoBrowser)

$ErrorActionPreference = 'Stop'
$server = $null
try {
    $Host.UI.RawUI.WindowTitle = 'Rusty VTT Alpha - serveur'
    $dataDirectory = Join-Path $env:LOCALAPPDATA 'Rusty VTT\data'
    New-Item -ItemType Directory -Path $dataDirectory -Force | Out-Null
    if (-not $env:RUSTY_DATA) { $env:RUSTY_DATA = Join-Path $dataDirectory 'world.json' }
    if (-not $env:RUSTY_BIND) { $env:RUSTY_BIND = '127.0.0.1:3000' }
    Write-Host 'Rusty VTT Alpha - gardez cette console ouverte pendant la partie.'
    Write-Host "Sauvegarde : $env:RUSTY_DATA"
    Write-Host 'Le secret MJ est affiche ci-dessous. Ctrl+C pour arreter.'
    # A second launch must not open another application's page on an occupied port.
    $address = $env:RUSTY_BIND
    $separator = $address.LastIndexOf(':')
    if ($separator -lt 1) { throw 'RUSTY_BIND invalide.' }
    $bindHost = $address.Substring(0, $separator).Trim('[', ']')
    $port = [int]$address.Substring($separator + 1)
    $probe = [System.Net.Sockets.TcpClient]::new()
    try {
        $probe.Connect($bindHost, $port)
        throw "Le port $port est deja utilise. Fermez l'autre serveur ou choisissez RUSTY_BIND."
    } catch [System.Net.Sockets.SocketException] {
        # No listener: the server can claim the configured address.
    } finally { $probe.Dispose() }
    $server = Start-Process -FilePath (Join-Path $PSScriptRoot 'rusty-vtt.exe') -NoNewWindow -PassThru
    if (-not $NoBrowser -and $env:RUSTY_HTTPS -ne '1') {
        $url = "http://$address"
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            if ($server.HasExited) { throw 'Le serveur a quitte avant de demarrer.' }
            try {
                $response = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 1
                if ($response.StatusCode -eq 200) {
                    Start-Process $url
                    break
                }
            } catch [System.Net.WebException] { }
            Start-Sleep -Milliseconds 250
        }
    }
    $server.WaitForExit()
    if ($server.ExitCode -ne 0) { throw "Le serveur s'est arrete (code $($server.ExitCode))." }
} catch {
    Write-Host $_.Exception.Message -ForegroundColor Red
    Read-Host 'Appuyez sur Entree pour fermer' | Out-Null
    exit 1
} finally {
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -ErrorAction SilentlyContinue }
}
