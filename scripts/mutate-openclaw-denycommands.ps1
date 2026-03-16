$cfg='C:\Users\Shaos\.openclaw\openclaw.json'
$j = Get-Content $cfg -Raw | ConvertFrom-Json
$j.gateway.nodes.denyCommands = @(
  'system.run',
  'canvas.present',
  'canvas.hide',
  'canvas.navigate',
  'canvas.eval',
  'canvas.snapshot',
  'canvas.a2ui.push',
  'canvas.a2ui.pushJSONL',
  'canvas.a2ui.reset'
)
$j | ConvertTo-Json -Depth 100 | Set-Content $cfg -Encoding UTF8
Write-Host 'mutated=gateway.nodes.denyCommands'
