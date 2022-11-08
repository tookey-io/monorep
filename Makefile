SHELL := /bin/bash

start_services:
	docker-compose -f docker-compose.yaml up

restart_services:
	docker-compose -f docker-compose.yaml down && docker-compose -f docker-compose.yaml up --build

start_dev:
	docker-compose -f docker-compose.dev.yaml -p dev up

restart_dev:
	docker-compose -f docker-compose.dev.yaml -p dev down && docker-compose -f docker-compose.dev.yaml -p dev up --build

.PHONY: test
.ONESHELL:
