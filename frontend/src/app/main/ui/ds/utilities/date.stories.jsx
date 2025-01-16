import * as React from "react";
import Components from "@target/components";

const { Date } = Components;

export default {
  title: "Foundations/Utilities/Date",
  component: Date,
  argTypes: {
    date: {
      control: { type: "date" },
    }
  },
  args: {
    title: "Date"
  },
  render: ({ ...args }) => <Date {...args}/>,
};

export const Default = {};

