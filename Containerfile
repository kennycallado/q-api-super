FROM busybox AS build-env

FROM scratch

# --build-arg PACKAGE_NAME=${package_name}
ARG PACKAGE_NAME="q-api-super" 
ARG TARGET="x86_64-unknown-linux-musl"

COPY ./target/${TARGET}/release/${PACKAGE_NAME} /bin/super
# COPY ./Rocket.toml /

COPY --from=build-env /tmp /tmp

# WORKDIR /
CMD [ "super" ]
