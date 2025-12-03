'use client';
import { type ComponentProps, Fragment, useMemo, useState } from 'react';
import { cva } from 'class-variance-authority';
import Link from 'fumadocs-core/link';
import { cn } from '../../../lib/cn';
import {
  type NavOptions,
  resolveLinkItems,
  type LinkItemType,
} from '../shared';
import { LinkItem } from '../link-item';
import {
  NavigationMenu,
  NavigationMenuContent,
  NavigationMenuItem,
  NavigationMenuLink,
  NavigationMenuList,
  NavigationMenuTrigger,
  NavigationMenuViewport,
} from '../../navigation-menu';
import { buttonVariants } from '../../ui/button';
import type { HomeLayoutProps } from './index';
import {
  LargeSearchToggle,
  SearchToggle,
} from '../search-toggle';
import { ThemeToggle } from '../theme-toggle';
import {
  LanguageToggle,
  LanguageToggleText,
} from '../language-toggle';
import { ChevronDown, Languages } from 'lucide-react';
import { useIsScrollTop } from 'fumadocs-ui/utils/use-is-scroll-top';

export const navItemVariants = cva('', {
  variants: {
    variant: {
      main: 'inline-flex items-center gap-1 text-sm text-gray-700 dark:text-[#d4c5b0] transition-colors hover:text-teal-700 dark:hover:text-teal-400 data-[active=true]:text-teal-700 dark:data-[active=true]:text-teal-400',
      button: 'inline-flex items-center bg-gradient-to-r from-teal-600 to-teal-700 hover:from-teal-700 hover:to-teal-800 text-white shadow-sm gap-1.5 px-4 py-2 rounded-lg text-sm transition-all [&_svg]:size-3.5',
      icon: 'p-2 text-gray-700 dark:text-[#d4c5b0] hover:bg-teal-50 dark:hover:bg-[#3a2d1d] rounded-lg transition-colors [&_svg]:size-4',
    },
  },
  defaultVariants: {
    variant: 'main',
  },
});

export function Header({
  nav = {},
  i18n = false,
  links,
  githubUrl,
  themeSwitch = {},
  searchToggle = {},
}: HomeLayoutProps) {
  const { navItems, menuItems } = useMemo(() => {
    const navItems: LinkItemType[] = [];
    const menuItems: LinkItemType[] = [];

    for (const item of resolveLinkItems({ links, githubUrl })) {
      switch (item.on ?? 'all') {
        case 'menu':
          menuItems.push(item);
          break;
        case 'nav':
          navItems.push(item);
          break;
        default:
          navItems.push(item);
          menuItems.push(item);
      }
    }

    return { navItems, menuItems };
  }, [links, githubUrl]);

  return (
    <HeaderNavigationMenu transparentMode={nav.transparentMode}>
      <Link
        href={nav.url ?? '/'}
        className="inline-flex items-center gap-2.5 font-semibold"
      >
        {nav.title}
      </Link>
      {nav.children}
      <ul className="flex flex-row items-center gap-8 ml-10 max-sm:hidden">
        {navItems
          .filter((item) => !isSecondary(item))
          .map((item, i) => (
            <NavigationMenuLinkItem key={i} item={item} className="text-sm" />
          ))}
      </ul>
      <div className="flex flex-row items-center justify-end gap-3 flex-1 max-lg:hidden">
        {themeSwitch.enabled !== false &&
          (themeSwitch.component ?? <ThemeToggle mode={themeSwitch?.mode} />)}
        {i18n && (
          <LanguageToggle>
            <Languages className="size-5" />
          </LanguageToggle>
        )}
        <ul className="flex flex-row gap-3 items-center empty:hidden">
          {navItems.filter(isSecondary).map((item, i) => (
            <NavigationMenuLinkItem
              key={i}
              item={item}
            />
          ))}
        </ul>
      </div>
      <ul className="flex flex-row items-center ms-auto -me-1.5 lg:hidden">
        {searchToggle.enabled !== false &&
          (searchToggle.components?.sm ?? (
            <SearchToggle className="p-2" hideIfDisabled />
          ))}
        <NavigationMenuItem>
          <NavigationMenuTrigger
            aria-label="Toggle Menu"
            className={cn(
              buttonVariants({
                size: 'icon',
                color: 'ghost',
                className: 'group [&_svg]:size-5.5',
              }),
            )}
            onPointerMove={
              nav.enableHoverToOpen ? undefined : (e) => e.preventDefault()
            }
          >
            <ChevronDown className="transition-transform duration-300 group-data-[state=open]:rotate-180" />
          </NavigationMenuTrigger>
          <NavigationMenuContent className="flex flex-col p-4 sm:flex-row sm:items-center sm:justify-end">
            {menuItems
              .filter((item) => !isSecondary(item))
              .map((item, i) => (
                <MobileNavigationMenuLinkItem
                  key={i}
                  item={item}
                  className="sm:hidden"
                />
              ))}
            <div className="-ms-1.5 flex flex-row items-center gap-2 max-sm:mt-2">
              {menuItems.filter(isSecondary).map((item, i) => (
                <MobileNavigationMenuLinkItem
                  key={i}
                  item={item}
                  className={cn(item.type === 'icon' && '-mx-1 first:ms-0')}
                />
              ))}
              <div role="separator" className="flex-1" />
              {i18n && (
                <LanguageToggle>
                  <Languages className="size-5" />
                  <LanguageToggleText />
                  <ChevronDown className="size-3 text-fd-muted-foreground" />
                </LanguageToggle>
              )}
              {themeSwitch.enabled !== false &&
                (themeSwitch.component ?? (
                  <ThemeToggle mode={themeSwitch?.mode} />
                ))}
            </div>
          </NavigationMenuContent>
        </NavigationMenuItem>
      </ul>
    </HeaderNavigationMenu>
  );
}

function isSecondary(item: LinkItemType): boolean {
  if ('secondary' in item && item.secondary != null) return item.secondary;

  return item.type === 'icon';
}

function HeaderNavigationMenu({
  transparentMode = 'none',
  ...props
}: ComponentProps<'div'> & {
  transparentMode?: NavOptions['transparentMode'];
}) {
  const [value, setValue] = useState('');
  const isTop = useIsScrollTop({ enabled: transparentMode === 'top' }) ?? true;
  const isTransparent =
    transparentMode === 'top' ? isTop : transparentMode === 'always';

  return (
    <NavigationMenu value={value} onValueChange={setValue} asChild>
      <div
        id="nd-nav"
        {...props}
        className={cn('fixed top-0 left-0 right-0 z-50 flex justify-center pt-6 px-4', props.className)}
      >
        <header
          className={cn(
            'transition-all duration-300 rounded-full',
            isTransparent 
              ? 'backdrop-blur-md bg-[#f5f1e8]/60 dark:bg-[#2a2118]/70 border border-[#d4c5b0]/20 dark:border-[#4a3828]/30' 
              : 'backdrop-blur-xl bg-[#f5f1e8]/80 dark:bg-[#2a2118]/90 shadow-lg border border-[#d4c5b0]/30 dark:border-[#4a3828]/40',
            value.length > 0 && 'max-lg:shadow-lg',
          )}
        >
          <NavigationMenuList
            className="flex h-14 w-full items-center px-8 py-4"
            asChild
          >
            <nav>{props.children}</nav>
          </NavigationMenuList>

          <NavigationMenuViewport />
        </header>
      </div>
    </NavigationMenu>
  );
}

function NavigationMenuLinkItem({
  item,
  ...props
}: {
  item: LinkItemType;
  className?: string;
}) {
  if (item.type === 'custom') return <div {...props}>{item.children}</div>;

  if (item.type === 'menu') {
    const children = item.items.map((child, j) => {
      if (child.type === 'custom') {
        return <Fragment key={j}>{child.children}</Fragment>;
      }

      const {
        banner = child.icon ? (
          <div className="w-fit rounded-md border bg-fd-muted p-1 [&_svg]:size-4">
            {child.icon}
          </div>
        ) : null,
        ...rest
      } = child.menu ?? {};

      return (
        <NavigationMenuLink key={`${j}-${child.url}`} asChild>
          <Link
            href={child.url}
            external={child.external}
            {...rest}
            className={cn(
              'flex flex-col gap-2 rounded-lg border bg-fd-card p-3 transition-colors hover:bg-fd-accent/80 hover:text-fd-accent-foreground',
              rest.className,
            )}
          >
            {rest.children ?? (
              <>
                {banner}
                <p className="text-base font-medium">{child.text}</p>
                <p className="text-sm text-fd-muted-foreground empty:hidden">
                  {child.description}
                </p>
              </>
            )}
          </Link>
        </NavigationMenuLink>
      );
    });

    return (
      <NavigationMenuItem {...props}>
        <NavigationMenuTrigger className={cn(navItemVariants(), 'rounded-md')}>
          {item.url ? (
            <Link href={item.url} external={item.external}>
              {item.text}
            </Link>
          ) : (
            item.text
          )}
        </NavigationMenuTrigger>
        <NavigationMenuContent className="grid grid-cols-1 gap-2 p-4 md:grid-cols-2 lg:grid-cols-3">
          {children}
        </NavigationMenuContent>
      </NavigationMenuItem>
    );
  }

  return (
    <NavigationMenuItem {...props}>
      <NavigationMenuLink asChild>
        <LinkItem
          item={item}
          aria-label={item.type === 'icon' ? item.label : undefined}
          className={cn(navItemVariants({ variant: item.type }))}
        >
          {item.type === 'icon' ? item.icon : item.text}
        </LinkItem>
      </NavigationMenuLink>
    </NavigationMenuItem>
  );
}

function MobileNavigationMenuLinkItem({
  item,
  ...props
}: {
  item: LinkItemType;
  className?: string;
}) {
  if (item.type === 'custom')
    return <div className={cn('grid', props.className)}>{item.children}</div>;

  if (item.type === 'menu') {
    const header = (
      <>
        {item.icon}
        {item.text}
      </>
    );

    return (
      <div className={cn('mb-4 flex flex-col', props.className)}>
        <p className="mb-1 text-sm text-fd-muted-foreground">
          {item.url ? (
            <NavigationMenuLink asChild>
              <Link href={item.url} external={item.external}>
                {header}
              </Link>
            </NavigationMenuLink>
          ) : (
            header
          )}
        </p>
        {item.items.map((child, i) => (
          <MobileNavigationMenuLinkItem key={i} item={child} />
        ))}
      </div>
    );
  }

  return (
    <NavigationMenuLink asChild>
      <LinkItem
        item={item}
        className={cn(
          {
            main: 'inline-flex items-center gap-2 py-1.5 transition-colors hover:text-fd-popover-foreground/50 data-[active=true]:font-medium data-[active=true]:text-fd-primary [&_svg]:size-4',
            icon: buttonVariants({
              size: 'icon',
              color: 'ghost',
            }),
            button: buttonVariants({
              color: 'secondary',
              className: 'gap-1.5 [&_svg]:size-4',
            }),
          }[item.type ?? 'main'],
          props.className,
        )}
        aria-label={item.type === 'icon' ? item.label : undefined}
      >
        {item.icon}
        {item.type === 'icon' ? undefined : item.text}
      </LinkItem>
    </NavigationMenuLink>
  );
}
