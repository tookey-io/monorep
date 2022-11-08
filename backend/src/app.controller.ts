import {
  Body,
  Controller,
  Delete,
  Get,
  Header,
  Post,
  Put,
  Query,
} from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PublicKey, Room, User } from './user';
import {
  AmqpConnection,
  RabbitPayload,
  RabbitSubscribe,
} from '@golevelup/nestjs-rabbitmq';

const users: Map<string, User> = new Map();
const keys: Map<string, PublicKey> = new Map();

{
  const key = new PublicKey();

  key.id = '4ed0357b-8fa9-4416-be2f-174575b9c900';
  key.room_id = '31694e3c-d98b-474d-a336-25243f3c8ce9';
  key.public_key = '';
  key.participants_count = 3;
  key.participant_index = 1;
  key.participants_threshold = 2;
  key.participants_confirmations = [1, 2, 3];
  key.status = 'created';
  key.rooms = new Map();

  keys.set(key.id, key);
}

function replacer(key, value) {
  if (value instanceof Map) {
    return {
      dataType: 'Map',
      value: Array.from(value.entries()), // or with spread: value: [...value]
    };
  } else {
    return value;
  }
}

@Controller()
export class AppController {
  constructor(private readonly amqp: AmqpConnection) {}

  @Post('/api/sign_up')
  @Header('Content-Type', 'application/json')
  signUp(@Body() body): string {
    const user = new User();

    user.id = body.user_id || randomUUID();
    user.email = body.email;
    user.public_keys = [];

    users.set(user.id, user);

    return JSON.stringify(user, replacer);
  }

  @Get('/api/user')
  @Header('Content-Type', 'application/json')
  getUser(@Query() params): string {
    return JSON.stringify(users.get(params.id), replacer);
  }

  @Get('/api/key')
  @Header('Content-Type', 'application/json')
  getKey(@Query() params): string {
    return JSON.stringify(keys.get(params.id), replacer);
  }

  @Put('/api/user')
  @Header('Content-Type', 'application/json')
  updateUser(@Body() params): string {
    const user = users.get(params.id);

    if (typeof user == 'undefined') {
      return 'Not found';
    }

    user.email = params.email;
    users.set(user.id, user);

    return JSON.stringify(users.get(params.id), replacer);
  }

  @Post('/api/public_keys')
  @Header('Content-Type', 'application/json')
  createKey(@Body() body): string {
    const user = users.get(body.user_id);

    if (typeof user == 'undefined') {
      return 'Not found';
    }

    const key = new PublicKey();

    key.id = body.public_key_id || randomUUID();
    key.room_id = body.public_key_room_id || randomUUID();
    key.public_key = null;
    key.participants_count = body.participants_count || 2;
    key.participant_index = body.participant_index || 1;
    key.participants_threshold = body.participants_threshold || 1;
    key.participants_confirmations = [];
    key.status = 'created';
    key.rooms = new Map();

    keys.set(key.id, key);
    user.public_keys.push(key.id);
    users.set(user.id, user);

    const message = {
      action: 'keygen_join',
      user_id: user.id,
      room_id: key.room_id,
      key_id: key.id,
      participant_index: key.participant_index,
      participants_count: key.participants_count,
      participants_threshold: key.participants_threshold - 1,
    };
    this.amqp.publish('amq.topic', 'manager', message);

    return JSON.stringify(user, replacer);
  }

  @Delete('/api/public_keys')
  @Header('Content-Type', 'application/json')
  deleteKey(@Body() body): string {
    const user = users.get(body.user_id);

    if (typeof user == 'undefined') {
      return 'Not found';
    }

    keys.delete(body.public_key_id);
    users.set(user.id, user);

    return JSON.stringify(user, replacer);
  }

  @Post('/api/sign')
  @Header('Content-Type', 'application/json')
  sign(@Body() body): string {
    const room = new Room();
    room.id = body.room_id || randomUUID();
    room.data = body.data || '';
    room.metadata = body.metadata || { example: 'data' };
    room.participant_indexes = body.participant_indexes;
    room.participants_confirmations = [];
    room.expires_at = 0;
    room.status = 'created';

    const key = keys.get(body.public_key_id);

    key.rooms.set(room.id, room);

    const message = {
      action: 'sign_approve',
      user_id: body.user_id,
      room_id: room.id,
      key_id: key.id,
      data: room.data,
      participants_indexes: room.participant_indexes,
    };
    this.amqp.publish('amq.topic', 'manager', message);

    return JSON.stringify(key, replacer);
  }

  @RabbitSubscribe({
    routingKey: 'backend',
    exchange: 'amq.topic',
    queue: 'backend',
  })
  rabbit(@RabbitPayload() body) {
    console.log('Rabbit message: ', body);

    if (body.action === 'keygen_status') {
      const key = [...keys.values()].find(
        (value) => value.room_id === body.room_id,
      );

      key.status = body.status;

      if (key.status === 'finished') {
        key.public_key = body.public_key;
        key.participants_confirmations = body.active_indexes;
      } else if (key.status === 'started') {
        key.participants_confirmations = body.active_indexes;
      }
    } else if (body.action === 'sign_status') {
      const key = [...keys.values()].find((value) =>
        value.rooms.has(body.room_id),
      );
      const room = key.rooms.get(body.room_id);

      room.status = body.status;

      if (room.status === 'finished') {
        room.result = body.result;
        room.participants_confirmations = body.active_indexes;
      } else if (key.status === 'started') {
        room.participants_confirmations = body.active_indexes;
      }
    }
  }
}
