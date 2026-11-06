# Node must match the version CI uses (setup-node 24.x) and package.json "engines".
FROM node:26.12.0-trixie-slim@sha256:e556a144463cc8dc53afe8de6d2fa0db5c132fd39ffa232c17641bd40df2bf11 AS build

WORKDIR /app

# .npmrc carries legacy-peer-deps and ignore-scripts.
COPY package.json package-lock.json .npmrc /app/
# NODE_ENV=production is set only after the install, because it would make
# npm ci skip the devDependencies that `npm run build` needs.
RUN npm ci --ignore-scripts

COPY . ./
ENV NODE_ENV=production
RUN npm run build

FROM nginx:1.30.6-alpine@sha256:e9b2d7624fd7957f4ed44ae0a272d93fb73aeb8512ba8916f8118ae6143fd115

WORKDIR /usr/share/nginx/html

COPY --from=build /app/out/ ./
