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
  public public_keys: string[];
}

export class PublicKey {
  public id: string;
  public room_id: string;
  public public_key: string;
  public participant_index: number;
  public participants_count: number;
  public participants_threshold: number;
  public rooms: Map<string, Room>;

  public status: 'created' | 'started' | 'finished' | 'error';
  public participants_confirmations: number[];
}

export class Room {
  public id: string;
  public data: string;
  public metadata: any;
  public participant_indexes: number[];
  public expires_at: number;
  public result: string;

  public status: 'created' | 'started' | 'finished' | 'error';
  public participants_confirmations: number[];
}
