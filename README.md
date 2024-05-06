Basic script to generate a list of video games I've played + short review.

To refresh the igdb token:

```
curl -X POST "https://id.twitch.tv/oauth2/token?client_id=${IGDB_TWITCH_CLIENT_ID}&client_secret=${IGDB_TWITCH_CLIENT_SECRET}&grant_type=client_credentials"
```
