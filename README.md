# Ziria
Blazing fast and memory-safe avatar service.

## Caching
To achieve blazing fast speeds and to not get rate-limited by Mojang, we apply the following caching technique for avatars:
* Optimize the 8px * 8px avatar by removing PNG magic (48 bytes saved).
* Store the optimized avatar in Redis with a 20 minute expiry.
* Resize the optimized avatar on request and add back PNG magic.

These techniques allow us to store approximately 5,000,000 avatars for every GB of Redis cache.

## Reliability
To ensure reliability of the service, we plan to store avatars in long term storage, which is slower but more cost efficient than storing in memory. If Mojang' servers ever go down, your avatar can still be served if you've used the service before. Long term storage should be updated only if a request has been made and the last update of long term storage is greater than 24 hours.

## API
We currently provide three endpoints:
* `/avatar/{uuid/username}/{size}/{helm}`
* `/skin/{uuid/username}/{size}`
* `/skin/{uuid/username}`

### Avatar
To use the avatar service, you can use the `/avatar/{uuid}/{size}/{helm}` endpoint, `helm` being the overlay/helmet layer of the skin.

### Skin
To use the skin service, you can use either the `/skin/{uuid}/{size}` endpoint, or the `/skin/{uuid}`, the latter returns the original image without any resizing.

## Deployment

Clone the repository into an accessible directory and build the image using `podman build -t ziria .` once that is done you can proceed with the Quadlet setup.

We use Quadlets, it turns Containerfiles into easily manageable systemd services. Copy the files in the `systemd` directory or soft-link them to `~/.config/containers/systemd/`. If you wish to use your own Redis-compatible K/V storage, then you can modify the ziria.container file to remove the requirements for the dragonfly container and ziria network.

After copying both the environment variables and Containerfile, we can reload the daemon using `systemctl --user daemon-reload` and start the service `systemctl --user start ziria.service`. You can then verify this by checking the status of it: `systemctl --user status ziria.service`.