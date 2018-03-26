# Data prefix

When converting a data set from a format to a NTFS dataset, a prefix can be added to all the identifiers. 
Prepending all the identifiers with a unique prefix ensures that the NTFS identifiers are unique accross all the NTFS datasets. With this assumption, merging two NTFS datasets can be done without worrying about conflicting identifiers.  

This prefix should be applied to all NTFS identifier except for the physical_mode identifiers that are standarized and fixed values. Fixed values are described in the [NTFS specifications](https://github.com/CanalTP/navitia/blob/dev/documentation/ntfs/ntfs_fr.md#physical_modestxt-requis)

