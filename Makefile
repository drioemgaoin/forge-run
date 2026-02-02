up:
	docker compose up -d

down:
	docker compose down

logs:
	docker compose logs -f app

migrate:
	docker compose run --rm migrate

up-observability:
	docker compose --profile observability up -d

down-observability:
	docker compose --profile observability stop
