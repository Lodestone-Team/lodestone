FROM node:21 as build

WORKDIR /app

ENV NODE_ENV production

COPY package*.json /app/
RUN npm ci --legacy-peer-deps

COPY . ./
RUN npm run build

FROM nginx

WORKDIR /usr/share/nginx/html

COPY --from=build /app/out/ ./
