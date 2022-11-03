/*
* - Users
  - id: UUID
  - email: String
  - telegram_id: i64
  - public_keys: {id: UUID, participant_number: u16, own_description: String}[]
- Public keys
  - id: UUID
  - public_key: String (use as uuid ?)
  - participants_number: u16
  - required_participants_number: u16
  - rooms: Room[]
- Rooms
  - id: UUID (for relay)
  - data: String (for relay)
  - metadata: JSON (for users and wallets)
  - participant_numbers: u16[]
  - public_key: String
  - expires_at: UTC Timestamp
  - status: { approved_participant_numbers: u16[] }
* */

export class User {
  public id: string;
  public email: string;
  public telegram_id: string;
  public public_keys: Map<
    string,
    { public_key: PublicKey; participant_number: number }
  >;
}

export class PublicKey {
  public id: string;
  public public_key: string;
  public participants_number: number;
  public required_participants_number: number;
  public rooms: Room[];
}

export class Room {
  public id: string;
  public data: string;
  public metadata: any;
  public participant_numbers: number[];
  public expires_at: number;
  public status: { finished: boolean; approved_participants_numbers: number[] };
  public result: string;
}
