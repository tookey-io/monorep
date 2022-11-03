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
import { AppService } from './app.service';
import { randomUUID } from 'crypto';
import { PublicKey, Room, User } from './user';

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
  constructor(private readonly appService: AppService) {}

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
    key.public_key = randomUUID();
    key.participants_number = body.participants_number || 3;
    key.required_participants_number = body.required_participants_number || 2;
    key.rooms = [];

    user.public_keys.set(key.id, {
      public_key: key,
      participant_number: 0,
    });
    users.set(user.id, user);

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

    key.public_key.rooms.push(room);
    user.public_keys.set(key.public_key.id, key);
    users.set(user.id, user);

    return JSON.stringify(user, replacer);
  }
}
