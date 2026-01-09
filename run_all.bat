@echo off
cd /d "%~dp0"

echo Starting Database...
docker-compose up -d

echo Waiting for DB...
timeout /t 5 /nobreak

echo Running Migrations...
sqlx migrate run --source backend/migrations --database-url postgres://admin:Ab13cba46def79_@127.0.0.1:5433/xfchess

echo Starting Backend...
start "XFChess Backend" cargo run -p backend

echo Starting Client...
start "XFChess Client" cargo run -p xfchess
