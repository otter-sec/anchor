cd packages;
for D in */;
    do if [ "$D" = "anchor/" ]; then
        cd $D && yarn --frozen-lockfile && yarn build; cd ..;
    else
        # `init:yarn` is a script defined in each package.json that already uses
        # `yarn --frozen-lockfile` internally — this is a script call, not an install.
        # locked-in: ignore[yarn-frozen-lockfile]
        cd $D && yarn run init:yarn; cd ..;
    fi
done
