SHELL := /bin/bash

start_services:
	docker-compose -f docker-compose.yaml up

restart_services:
	docker-compose -f docker-compose.yaml down && docker-compose -f docker-compose.yaml up --build

.PHONY: test
.ONESHELL:
