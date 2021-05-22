for i in {0..99}
do
  node=onyxnode$(printf "%02d" $i)
  echo $node
  echo yes | ssh $node pwd
done
