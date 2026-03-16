$cfg='C:\Users\Shaos\.openclaw\openclaw.json'
$j = Get-Content $cfg -Raw | ConvertFrom-Json
$j.channels.discord.token = '${DISCORD_BOT_TOKEN}'
$j.gateway.auth.token = '${OPENCLAW_GATEWAY_TOKEN}'
$j | ConvertTo-Json -Depth 100 | Set-Content $cfg -Encoding UTF8
Write-Host 'mutated=openclaw.json token fields -> env vars'
