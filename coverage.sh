ssh onyx "
cd ~/research/thesis/aurum;
git pull;
cargo test --no-run;
cargo tarpaulin --exclude-files code-gen/* --exclude-files tests/* --exclude-files src/macros/* --out Xml;
"
scp onyx:~/research/thesis/aurum/cobertura.xml .
pycobertura show --format html --output coverage.html cobertura.xml
/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome coverage.html

