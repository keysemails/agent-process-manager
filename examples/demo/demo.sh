#!/bin/bash
# Demo script showing how Agent Process Manager works

echo "=== Agent Process Manager Demo ==="
echo ""
echo "1. Starting the APM daemon:"
echo "   \$ apm start"
echo ""
echo "2. Starting processes:"
echo "   \$ apm start frontend \"npm run dev\" --tag=frontend,dev"
echo "   \$ apm start backend \"python manage.py runserver\" --tag=backend,api"
echo ""
echo "3. AI Agent queries (minimal context):"
echo ""
echo "   Instead of: tmux capture-pane -t dev -p | tail -100"
echo "   Use: curl localhost:7337/api/processes/frontend/logs?format=summary"
echo ""
echo "   Returns:"
cat << 'EOF'
   {
     "status": "running",
     "uptime": "5m 23s",
     "key_events": [
       "Server started on http://localhost:3000",
       "Connected to database",
       "Compiled successfully in 2.3s"
     ],
     "recent_errors": [],
     "detected_urls": ["http://localhost:3000"],
     "detected_ports": [3000],
     "resource_usage": { "cpu": "12%", "memory": "234MB" }
   }
EOF
echo ""
echo "4. Human access (full logs):"
echo "   \$ apm attach frontend  # Like tmux attach"
echo "   \$ apm logs frontend --follow"
echo "   \$ open http://localhost:7337/dashboard"
echo ""
echo "5. Benefits:"
echo "   - 90% less context usage for AI agents"
echo "   - Automatic pattern detection (ports, URLs, errors)"
echo "   - Works with any language/framework"
echo "   - Preserves full logs for humans"
echo ""