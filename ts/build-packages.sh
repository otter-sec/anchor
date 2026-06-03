cd packages;
for D in */;
    do if [ "$D" = "anchor/" ]; then
        cd $D && yarn --frozen-lockfile && yarn build; cd ..;
    else
        cd $D && yarn init:yarn; cd ..;
    fi
done
