import React from 'react';
import { render } from '@testing-library/react';
import DashboardCard from '../src/components/DashboardCard';

jest.mock('utils/util', () => ({
  cn: jest.fn((...args) => args.join(' ')),
}));

describe('DashboardCard Component', () => {
  it('renders children with specified className', () => {
    const mockChildren = <div>Mock Children</div>; // Replace with your mock children

    const { getByText } = render(<DashboardCard>{mockChildren}</DashboardCard>);
    const childrenElement = getByText('Mock Children');

    expect(childrenElement).toBeInTheDocument(); // Ensure children are rendered
    expect(childrenElement.parentElement).toHaveClass('flex h-fit w-full flex-col justify-evenly gap-8 rounded-2xl border border-gray-faded/30 bg-gray-850 p-4'); // Ensure parent container has correct className
  });

  it('renders children with additional className', () => {
    const mockChildren = <div>Mock Children</div>; // Replace with your mock children

    const { getByText } = render(<DashboardCard className="additional-class">{mockChildren}</DashboardCard>);
    const childrenElement = getByText('Mock Children');

    expect(childrenElement).toBeInTheDocument(); // Ensure children are rendered
    expect(childrenElement.parentElement).toHaveClass('flex h-fit w-full flex-col justify-evenly gap-8 rounded-2xl border border-gray-faded/30 bg-gray-850 p-4 additional-class'); // Ensure parent container has correct className including additional class
  });

});