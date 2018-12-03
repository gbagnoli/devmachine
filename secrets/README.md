# SECRETS

Place here the .json files for the secrets, before running

i.e. 

```
./run -H node1
```

will search for a file named `node1.json` in this directory

Files are just passed to chef as attribute files (i.e. `chef-client -j <file>`)
