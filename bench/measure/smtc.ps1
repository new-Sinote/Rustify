$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
})[0]
function Await($WinRtTask, $ResultType) {
  $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
  $netTask = $asTask.Invoke($null, @($WinRtTask))
  $netTask.Wait(-1) | Out-Null
  $netTask.Result
}

$managerType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType = WindowsRuntime]
$propsType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties, Windows.Media.Control, ContentType = WindowsRuntime]
$manager = Await ($managerType::RequestAsync()) $managerType
$session = $manager.GetCurrentSession()
if (-not $session) {
  [pscustomobject]@{ session = $false } | ConvertTo-Json -Compress
  exit 0
}
$props = Await ($session.TryGetMediaPropertiesAsync()) $propsType
$info = $session.GetPlaybackInfo()
$timeline = $session.GetTimelineProperties()
[pscustomobject]@{
  session = $true
  app = $session.SourceAppUserModelId
  title = $props.Title
  artist = $props.Artist
  album = $props.AlbumTitle
  status = [string]$info.PlaybackStatus
  position_s = [math]::Round($timeline.Position.TotalSeconds, 1)
  end_s = [math]::Round($timeline.EndTime.TotalSeconds, 1)
  at = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json -Compress
