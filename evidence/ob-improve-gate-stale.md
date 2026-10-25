Defect: plugin reverse-engineer-anything — le process gate (MCP server lancé par le démon) survit aux mises à jour du code du plugin : le nouveau code du gate (ex. injection nativeEnv GHIDRA_INSTALL_DIR ajoutée en beta-2) reste inactif tant que le process stale tourne, et `rea_cli doctor` rapporte alors faussement « GHIDRA_INSTALL_DIR is not set » alors que /opt/ghidra/current existe et que le doctor direct (avec env) est vert.

Repro observé 2026-10-22 (session Diablo 2 WASM) : gate démarré 02:32 (pid 14493), fichier gate.ts mis à jour 04:01:31 ; doctor via rea_cli => ghidra missing ; doctor bash avec GHIDRA_INSTALL_DIR explicite => ghidra healthy 12.1.4 ; kill du pid stale + re-call => ghidra healthy via le gate.

Fix suggéré : le gate inscrit un stamp de version/hash du fichier dans gate.log au démarrage et se re-vérifie (ou le daemon respawn le gate quand le mtime du fichier change) ; le message d'erreur du doctor pourrait aussi mentionner « gate process démarré avant la dernière mise à jour du plugin ».

Note secondaire : le lock est global — une session REA ouverte depuis un autre thread a fermé la mienne en cours d'analyse (04:01, log : mcp backend stopped sigterm suivi d'un doctor d'un autre process) ; un message d'état « fermé par une autre session à HH:MM » aiderait au débogage.
