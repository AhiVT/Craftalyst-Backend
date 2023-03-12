ALTER TABLE `minecrafters`
ADD COLUMN `suspended` TINYINT NOT NULL DEFAULT 0 AFTER `minecraft_name`;
