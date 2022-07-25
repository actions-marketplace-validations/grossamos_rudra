# Rudra
Rudra is an openapi based test coverage analysis tool.
It allows teams to set and enforce coverage levels for integration tests in CI/CD-pipelines.

**NOTE: rudra is still under development and not yet production ready**

## Quickstart
#### Step 1: Point your integration tests to rudra
For rudra to work, you have to point your integration tests to the rudra reverse proxy.
In postman for example this can be done by creating a new environment and modifying a base url.

Point your tests towards `http://localhost:13750` or `http://rudra:13750`.

#### Step 2: Add a configuration step
Place the rudra preperation stage **after** your service is running and **before** you'll run your integration tests.

```yaml
  - name: init rudra
    uses: grossamos/rudra@v0.1.0
    with:
      stage: "preperation"
      openapi-source: "docs/swagger.json"
      instance-url: "http://localhost:8080"
      test-coverage: "75%"
```

Modify `openapi-source` to point to your openapi/swagger specification. This can also be a url.

Modify `instance-url` to point to the base of your service (everything before the basepath of your openapi spec).

Optionally set a desired `test-coverage` for your endpoints.

#### Step 3: Add evaluation step
Place the rudra evaluation stage somewhere after your integration tests have run.

```yaml
  - uses: grossamos/rudra@v0.0.5-4
    name: eval rudra
    with:
      stage: "evaluation"
```
This stage will fail if test coverage isn't met and can display additional information gathered during the integration tests.

## Overview
Rudra works by acting as a reverse proxy between your application and integration tests.
It collects and compares the requests (and responses) with an openapi spec.

The reverse proxy is set up an configured in the first "preperation" stage.
Analysis and any propagation of results occurs during the "evaluation" stage.

### Configuration options
Option               | Description                                                                    | Values                      | Examples
---------------------|--------------------------------------------------------------------------------|-----------------------------|-----------------------
stage                | Specifies which stage to use                                                   | `preperation`, `evaluation` | `preperation`
openapi-source       | Location of openapi/swagger spec                                               | Path or URL                 | `docs/swagger.yaml`
instance-url         | Base of service, excluding basepath from openapi                               | URL                         | `http://localhost:8080`
debug                | Enables Debug mode (default off)                                               | boolean                     | `true`
account-for-security | Take security annotations into account and require 401/403 cases (default off) | boolean                     | `true`
test-coverage        | Coverage to enforce in evaluation stage                                        | Percentage or float         | `0.75`, `75%`

## Examples
A reference pipeline can be point under <https://github.com/grossamos/rudra-example>.
It uses a go service and postman to serve as an example of how to integrate rudra into your application.

## Local setup
### Install nix
Rudra uses nix for its dependency management, to get started install it and enable flakes.

#### On GNU/Linux systems or MacOS
Install nix onto your system
```bash
curl -L https://nixos.org/nix/install | sh
```

Enable flakes by adding the following line to `~/.config/nix/nix.conf` or `/etc/nix/nix.conf` (preferably the first).
```
experimental-features = nix-command flakes
```

#### On NixOS 
Enable flakes by adding the following options to your `nix.conf`
```nix
{ pkgs, ... }: {
  nix = {
    package = pkgs.nixFlakes;
    extraOptions = ''
      experimental-features = nix-command flakes
    '';
   };
}
```

### Build rudra
Rudra can then be built using nix:
```bash
nix build .
```

For development, rudra uses a nix shell.
The nix shell can be opened via:
```bash
nix develop
```

Rudras docker container can be built using docker (this will be migrated to a nix workflow in the future);
```bash
docker build -t rudra .
```

A typical testing environment would include `rudra-example` running as `app` in the rudra network.
This setup can be emulated by running:
```bash
docker network create rudra

docker run --name=app --network=rudra -d --rm rudra-example

docker run --env RUDRA_APP_BASE_URL=http://app:8080 --env RUDRA_OPENAPI_SOURCE=/swagger.yaml --volume $PWD/test/resource/swagger.yaml:/swagger.yaml -p 13750:80 --network rudra --name rudra --rm --env RUDRA_DEBUG=0 --env RUDRA_ACCOUNT_FOR_SECURITY=1 rudra
```

