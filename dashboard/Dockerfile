# Node must match the version CI uses (setup-node 24.x) and package.json "engines".
FROM node:24.22.1-trixie-slim@sha256:89c5244bc22a9bc3287d89ebc7e49b9cb5bb84cd40e6175ac3421d5a9f68b95a AS build

WORKDIR /app

# .npmrc carries legacy-peer-deps and ignore-scripts.
COPY package.json package-lock.json .npmrc /app/
# NODE_ENV=production is set only after the install, because it would make
# npm ci skip the devDependencies that `npm run build` needs.
RUN npm ci --ignore-scripts

COPY . ./
ENV NODE_ENV=production
RUN npm run build

FROM nginx:1.31.7-alpine@sha256:892bb2152464f0c5d8972593a5f5afaf8054552580b699d95f8ff4af08c2d33d

WORKDIR /usr/share/nginx/html

COPY --from=build /app/out/ ./
