folder="../../../../diem-move/diem-framework/core/sources"
for file in $(ls $folder);
do
  . ~/.profile
  cargo run -- $folder/$file -d ../../../../language/move-stdlib/sources -d $folder
done