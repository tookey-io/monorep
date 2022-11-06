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

  public status: 'created' | 'started' | 'finished' | 'error' | 'timeout';
  public participants_confirmations: number[];
}

export class Room {
  public id: string;
  public data: string;
  public metadata: any;
  public participant_indexes: number[];
  public expires_at: number;
  public result: string;

  public status: 'created' | 'started' | 'finished' | 'error' | 'timeout';
  public participants_confirmations: number[];
}
