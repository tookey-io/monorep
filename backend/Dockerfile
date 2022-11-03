FROM node:16-alpine

ENV NODE_ENV=development

WORKDIR /app
RUN chown 1000:1000 -R /app

USER 1000

COPY package.json yarn.lock /app/
RUN yarn --frozen-lockfile

COPY nest-cli.json tsconfig.build.json tsconfig.json /app/

CMD /bin/sh -c 'yarn run start:dev -- --preserveWatchOutput'
