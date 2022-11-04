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
  RabbitRPC,
  RabbitSubscribe,
} from '@golevelup/nestjs-rabbitmq';

const users: Map<string, User> = new Map();

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
    user.public_keys = new Map();

    users.set(user.id, user);

    return JSON.stringify(user, replacer);
  }

  @Get('/api/user')
  @Header('Content-Type', 'application/json')
  getUser(@Query() params): string {
    return JSON.stringify(users.get(params.id), replacer);
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
    key.public_key = '';
    key.participants_count = body.participants_count || 2;
    key.participant_index = body.participant_index || 1;
    key.participants_threshold = body.participants_threshold || 1;
    key.rooms = [];

    user.public_keys.set(key.id, key);
    users.set(user.id, user);

    const message = {
      action: 'keygen_join',
      user_id: user.id,
      room_id: key.id,
      participant_index: key.participant_index,
      participants_count: key.participants_count,
      participants_threshold: key.participants_threshold,
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

    user.public_keys.delete(body.public_key_id);
    users.set(user.id, user);

    return JSON.stringify(user, replacer);
  }

  @Post('/api/sign')
  @Header('Content-Type', 'application/json')
  sign(@Body() body): string {
    const user = users.get(body.user_id);

    if (typeof user == 'undefined') {
      return 'Not found';
    }

    const room = new Room();
    room.id = randomUUID();
    room.data = body.data || '';
    room.metadata = body.metadata || { example: 'data' };
    room.participant_numbers = [0, 1];
    room.expires_at = 0;
    room.status = { finished: false, approved_participants_numbers: [0] };

    const key = user.public_keys.get(body.public_key_id);

    key.rooms.push(room);
    user.public_keys.set(key.id, key);
    users.set(user.id, user);

    return JSON.stringify(user, replacer);
  }

  @RabbitSubscribe({
    routingKey: 'backend',
    exchange: 'amq.topic',
    queue: 'backend',
  })
  rabbit(@RabbitPayload() body) {
    console.log('Rabbit message: ', body);

    if (body.action === 'keygen_status') {
      const user = users.get(body.user_id);
      const key = user.public_keys.get(body.room_id);

      if (defined(body.public_key) && body.public_key.length > 0)
        key.public_key = body.public_key;

      if (!key.finished) key.finished = body.finished;

      if (body.active_indexes.length > 0)
        key.participants_confirmations = body.active_indexes;
    }
  }
}

function defined(thing): boolean {
  return typeof thing === 'undefined';
}
