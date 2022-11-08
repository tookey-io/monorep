# Tookey mono-repository

Tookey is assets and access management protocol for web3. We build secure environment to interact with crypto without
risk of disclose the private key.

# Notes

Services in `docker-compose.yaml` store state in memory, to reset state restart all containers. 

Threshold is amount of participants required for sign process. In backend request should be `i`, in console cli `i - 1`.

Room values in `Running` are from postman collections default values.

# Running

To develop backend:

1. Run `make start_services` in one terminal
2. Run `make start_dev` in other terminal

### Key Generation for 3 parties and threshold is 1 (at least two key to sign)

1. Start docker containers (`make start_services` and `make start_dev`)
2. Open 3 terminals and run:
    1. `docker exec 2k_relay cargo run --bin keygen -- -t 1 -n 3 -i 1 --output keys/key1.json`
    2. `docker exec 2k_relay cargo run --bin keygen -- -t 1 -n 3 -i 2 --output keys/key2.json`
    3. `docker exec 2k_relay cargo run --bin keygen -- -t 1 -n 3 -i 3 --output keys/key3.json`

### Sign a message

1. Ensure you passed key Generation
2. Start docker containers (`make start_services` and `make start_dev`)
3. Open 2 terminals and run:
    1. `docker exec 2k_relay cargo run --bin sign -- -p 1,2 -h "0xbd621a5652a421f0b853d2a56609bfd26ae965709070708a34f7607f1ce97a60" -l keys/key1.json`
    2. `docker exec 2k_relay cargo run --bin sign -- -p 1,2 -h "0xbd621a5652a421f0b853d2a56609bfd26ae965709070708a34f7607f1ce97a60" -l keys/key2.json`

### Key Generation with backend and Simulated wallet with backup key

1. Start docker containers (`make start_services` and `make start_dev`)
2. Start backend (`cd backend && yarn run start:dev`)
3. Sign up with `POST /api/sign_up`
4. Start key generation with `POST /api/public_keys`
5. Verify key state with `GET /api/key`
6. Join wallet key generation with command in
   terminal `docker exec 2k_relay cargo run --bin keygen -- -t 1 -n 3 -i 2 --room '31694e3c-d98b-474d-a336-25243f3c8ce9' --output keys/key2.json`
7. Join backup key generation with command in
   terminal `docker exec 2k_relay cargo run --bin keygen -- -t 1 -n 3 -i 3 --room '31694e3c-d98b-474d-a336-25243f3c8ce9' --output keys/key3.json`
8. Generation commands should successfully exit and create files in `keys` folder
9. Verify key state with 'GET /api/key'

On key generation errors try restarting docker containers

### Hash signing with backend and Simulated wallet

1. Start docker containers (`make start_services` and `make start_dev`)
2. Start backend (`cd backend && yarn run start:dev`)
3. Complete key generation
4. Verify key state with `GET /api/key`
5. Start sign process with `POST /api/sign`
6. Join sign process with command in
   terminal `docker exec 2k_relay cargo run --bin sign -- -p 1,2 -h "0xbd621a5652a421f0b853d2a56609bfd26ae965709070708a34f7607f1ce97a60" --room 'a8b451fe-5e4e-4ace-aaba-683d1467d9a5' -l keys/key2.json`
7. Sign commands should successfully exit and output result in console
8. Verify sign state with 'GET /api/key' (sign should have status: finished and output in output field)

On sign errors try restarting docker containers, verify correct threshold numbers
