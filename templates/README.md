# {{name}}

{{description}}

## Developing

```sh
lorealm-plugin build
```

Then, in Lorealm, install it with "Add from source" and this folder.

## Releasing

```sh
lorealm-plugin stage 0.2.0   # sets the version, commits it and tags it
lorealm-plugin publish       # pushes the tag, which builds the release
```
