import Image from "next/image";
import type { PublicConfig } from "@/src/lib/config";

type BrandLogoProps = {
  config: PublicConfig;
  className: string;
  width: number;
  height: number;
  priority?: boolean;
};

export function BrandLogo({ config, className, width, height, priority }: BrandLogoProps) {
  return (
    <>
      <Image
        alt={config.brand.name}
        className={`${className} dark:hidden`}
        height={height}
        priority={priority}
        src={config.brand.logoAsset}
        width={width}
      />
      <Image
        alt={config.brand.name}
        className={`hidden ${className} dark:block`}
        height={height}
        priority={priority}
        src={config.brand.logoAssetDark}
        width={width}
      />
    </>
  );
}
